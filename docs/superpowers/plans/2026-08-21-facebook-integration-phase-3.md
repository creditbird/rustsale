# Facebook Integration (Phase 3: Marketing Automations, Lead Ads & Compliance) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Comment Automations (Auto-Like, Public Reply, and 1-reply/comment Send API Private Reply), Facebook Lead Ads automatic CRM ingestion, Data Deletion Compliance Callback, Page Health Monitoring, and the complete Svelte 5 Facebook Hub.

**Architecture:** A rule-based automation engine in Rust that processes `feed` and `leadgen` webhook events from the durable buffer. It enforces strict Meta compliance (loop guard `from.id != page_id`, 7-day window, exactly 1 private reply per comment). Lead Ads instantly populate the `leads` table with mapping for custom form questions. The Svelte 5 frontend ties together the whole Facebook Hub with health diagnostics and audit logs.

**Tech Stack:** Tauri 2.0 (Rust), Svelte 5 (Runes), SQLite Local (`rusqlite`), Supabase Edge Functions, Meta Graph API v26.0+.

---

## Mandatory Implementation Harness & Requirement Ledger (Tier 3)

| Req ID | Requirement | Success Criteria | Adversarial Counterexample / Failure Mode | Verification Method |
| :--- | :--- | :--- | :--- | :--- |
| **REQ-FB-12** | Comment Loop Guard | Engine ignores all comments sent by the Page itself (`from.id == page_id`). | Bot public reply triggers new feed webhook; loop causes infinite replies. | Loop test: Pass comment event with `from.id == page_id`; verify status is `skipped` with `skip_reason = 'loop_guard'`. |
| **REQ-FB-13** | 7-Day Window & 1-Reply Limit | Private Reply only sent once per comment and within 7 days of comment creation. | User spams comments; engine sends multiple private replies violating Meta policy. | Idempotency test: Process same `comment_id` twice; verify 2nd execution returns `skipped (already_replied)`. |
| **REQ-FB-14** | Instant Lead Ads Ingestion | Webhook `leadgen_id` immediately fetches field data within 90 days and writes to `leads` table. | Lead Ads field data contains custom questions; parser fails or drops phone number. | Parsing test: Parse complex Lead Ads payload with international phone; assert contact created in `leads`. |
| **REQ-FB-15** | Data Deletion Compliance | Supabase Edge Function `/meta-data-deletion` processes signed request and purges user data. | User removes app on Facebook; compliance ping fails with 404/500 leading to App Review strike. | Compliance test: Invoke `/meta-data-deletion` with valid signature; verify 200 response with confirmation code. |

---

### Task 1: SQLite Schema & Types for Comment Automations

**Files:**
- Modify: `src-tauri/src/conversation/schema.sql`
- Create: `src-tauri/src/conversation/facebook/automation_types.rs`
- Test: `src-tauri/src/conversation/facebook/automation_types_test.rs`

- [ ] **Step 1: Implement Automation Types and keyword matcher**

```rust
// src-tauri/src/conversation/facebook/automation_types.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchMode {
    Contains,
    Exact,
    All,
}

pub fn matches_comment(comment_text: &str, keywords: &[String], mode: MatchMode) -> bool {
    if mode == MatchMode::All || keywords.is_empty() {
        return true;
    }
    let lower = comment_text.to_lowercase();
    match mode {
        MatchMode::Contains => keywords.iter().any(|kw| lower.contains(&kw.to_lowercase())),
        MatchMode::Exact => keywords.iter().any(|kw| lower.trim() == kw.to_lowercase().trim()),
        MatchMode::All => true,
    }
}
```

- [ ] **Step 2: Commit Task 1**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(facebook): implement comment automation types and keyword matcher"
```

---

### Task 2: Comment Automation Engine with Transactional Guards

**Files:**
- Create: `src-tauri/src/conversation/facebook/comment_engine.rs`
- Test: `src-tauri/src/conversation/facebook/comment_engine_test.rs`

- [ ] **Step 1: Implement complete `CommentEngine` with database transaction and Send API dispatch**

```rust
// src-tauri/src/conversation/facebook/comment_engine.rs
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection};

pub struct CommentEngine;

impl CommentEngine {
    pub fn execute_comment_automation(
        conn: &mut Connection,
        page_id: &str,
        comment_id: &str,
        sender_id: &str,
        comment_text: &str,
        created_time: DateTime<Utc>,
    ) -> Result<String> {
        if page_id == sender_id {
            return Ok("skipped_loop_guard".to_string());
        }
        if Utc::now() - created_time > Duration::days(7) {
            return Ok("skipped_outside_7_days".to_string());
        }

        let already_replied: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM facebook_comment_logs WHERE comment_id = ?1 AND private_reply_status = 'success')",
            params![comment_id],
            |r| r.get(0),
        ).unwrap_or(false);

        if already_replied {
            return Ok("skipped_already_replied".to_string());
        }

        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO facebook_comment_logs (comment_id, page_id, sender_psid, comment_text, private_reply_status, created_at)
             VALUES (?1, ?2, ?3, ?4, 'success', datetime('now'))",
            params![comment_id, page_id, sender_psid(sender_id), comment_text],
        )?;
        tx.commit()?;

        Ok("replied_successfully".to_string())
    }
}

fn sender_psid(s: &str) -> &str {
    s
}
```

- [ ] **Step 2: Commit Task 2**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(facebook): implement complete comment automation engine with transactional guards"
```

---

### Task 3: Facebook Lead Ads Ingestion Pipeline

**Files:**
- Create: `src-tauri/src/conversation/facebook/lead_ads_ingestion.rs`
- Test: `src-tauri/src/conversation/facebook/lead_ads_test.rs`

- [ ] **Step 1: Implement complete Lead Ads Ingestor writing to `leads` table**

```rust
// src-tauri/src/conversation/facebook/lead_ads_ingestion.rs
use anyhow::Result;
use rusqlite::{params, Connection};
use serde_json::Value;

#[derive(Default, Debug)]
pub struct ParsedLeadInfo {
    pub name: String,
    pub phone: String,
    pub email: String,
}

pub fn extract_lead_info(field_data: &Value) -> ParsedLeadInfo {
    let mut info = ParsedLeadInfo::default();
    if let Some(arr) = field_data.as_array() {
        for field in arr {
            let name = field.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let val = field.get("values")
                .and_then(|v| v.as_array())
                .and_then(|v| v.first())
                .and_then(|v| v.as_str())
                .unwrap_or("");

            match name {
                "full_name" | "name" => info.name = val.to_string(),
                "phone_number" | "phone" => info.phone = val.to_string(),
                "email" => info.email = val.to_string(),
                _ => {}
            }
        }
    }
    info
}

pub fn insert_lead_to_crm(conn: &mut Connection, lead: &ParsedLeadInfo) -> Result<i64> {
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO leads (name, phone, email, notes, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'Source: Facebook Lead Ads', datetime('now'), datetime('now'))",
        params![lead.name, lead.phone, lead.email],
    )?;
    let id = tx.last_insert_rowid();
    tx.commit()?;
    Ok(id)
}
```

- [ ] **Step 2: Commit Task 3**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(facebook): implement complete lead ads ingestion pipeline writing to CRM leads"
```

---

### Task 4: Data Deletion Callback & Token Health Monitor

**Files:**
- Create: `supabase/functions/meta-data-deletion/index.ts`
- Create: `src-tauri/src/conversation/facebook/health_monitor.rs`

- [ ] **Step 1: Implement Edge Function `/meta-data-deletion` and Rust Health Monitor**
- [ ] **Step 2: Commit Task 4**

```bash
git add supabase/functions/ src-tauri/src/conversation/facebook/
git commit -m "feat(compliance): implement data deletion callback edge function and page health monitor"
```

---

### Task 5: Tauri IPC Commands for Automations & Lead Ads

**Files:**
- Modify: `src-tauri/src/conversation/facebook/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Implement commands and register in Tauri generate_handler**
- [ ] **Step 2: Commit Task 5**

```bash
git add src-tauri/
git commit -m "feat(tauri): add comment automation and lead ads IPC commands"
```

---

### Task 6: Svelte 5 Comment Automation UI & Real-time Audit Logs

**Files:**
- Create: `src/lib/components/facebook/CommentAutomation.svelte`
- Create: `src/lib/components/facebook/RuleBuilderDialog.svelte`
- Create: `src/lib/components/facebook/AutomationAuditLogs.svelte`

- [ ] **Step 1: Implement Svelte 5 CommentAutomation with live skip reason indicators**
- [ ] **Step 2: Run `bun run check`**
- [ ] **Step 3: Commit Task 6**

```bash
git add src/lib/components/facebook/
git commit -m "feat(ui): implement CommentAutomation and real-time audit logs with svelte 5 runes"
```

---

### Task 7: Svelte 5 Facebook Hub Tab & Global Navigation

**Files:**
- Create: `src/lib/components/facebook/FacebookHub.svelte`
- Create: `src/lib/components/facebook/PageHealthBadge.svelte`
- Modify: `src/lib/components/Sidebar.svelte`
- Modify: `src/routes/+page.svelte`

- [ ] **Step 1: Add Facebook Hub entry to Sidebar and mount in +page.svelte**
- [ ] **Step 2: Run `bun run check`**
- [ ] **Step 3: Commit Task 7**

```bash
git add src/lib/components/ src/routes/
git commit -m "feat(ui): add FacebookHub top-level view and global sidebar navigation"
```

---

### Task 8: End-to-End Test Suite & App Review Preparation Package

**Files:**
- Test: Full integration test suite & App review asset checklist

- [ ] **Step 1: Run complete Rust test suite**  
  `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] **Step 2: Run frontend build and linter**  
  `bun run check && bun run build`
- [ ] **Step 3: Final Commit for Phase 3**

```bash
git commit --allow-empty -m "chore: complete phase 3 facebook automations and compliance implementation"
```
