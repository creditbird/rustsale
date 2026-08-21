# Facebook Integration (Phase 3: Marketing Automations, Lead Ads & Compliance) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Comment Automations (Auto-Like, Public Reply, and 1-reply/comment Send API Private Reply), Facebook Lead Ads automatic CRM ingestion, Data Deletion Compliance Callback, Page Health Monitoring, and the complete Svelte 5 Facebook Hub.

**Architecture:** A rule-based automation engine in Rust that processes `feed` and `leadgen` webhook events from the durable buffer. It enforces strict Meta compliance (loop guard `from.id != page_id`, 7-day window, exactly 1 private reply per comment). Lead Ads instantly populate the `leads` table with mapping for custom form questions. The Svelte 5 frontend ties together the whole Facebook Hub with health diagnostics and audit logs.

**Tech Stack:** Tauri 2.0 (Rust), Svelte 5 (Runes), SQLite Local (`rusqlite`), Supabase Edge Functions, Meta Graph API v26.0+.

## Global Constraints

- Always use `bun` and `bunx` instead of `node`, `npm`, `npx`.
- Private reply must use Send API: `POST /{page_id}/messages` with `recipient: {"comment_id": "..."}` and `messaging_type: "RESPONSE"`.
- Exactly 1 Private Reply per comment must be enforced via `facebook_comment_logs`.
- Lead Ads field data must be ingested immediately via `GET /{leadgen_id}` (valid within 90 days of creation).
- App Review Data Deletion Callback endpoint must be implemented on Supabase Edge Functions.
- TDD required: Write failing tests before implementation.

---

### Task 1: SQLite Schema & Types for Comment Automations

**Files:**
- Modify: `src-tauri/src/conversation/schema.sql`
- Create: `src-tauri/src/conversation/facebook/automation_types.rs`
- Modify: `src-tauri/src/conversation/facebook/mod.rs`
- Test: `src-tauri/src/conversation/facebook/automation_types_test.rs`

**Interfaces:**
- Produces:
  - Table `facebook_comment_automations`
  - Table `facebook_comment_logs`
  - Struct `CommentAutomationRule`
  - Struct `CommentAuditLog`
  - Enum `SkipReason` (`LoopGuard`, `Outside7Days`, `AlreadyReplied`)

- [ ] **Step 1: Write test for Keyword Matcher**

```rust
// src-tauri/src/conversation/facebook/automation_types_test.rs
#[cfg(test)]
mod tests {
    use super::super::automation_types::*;

    #[test]
    fn test_keyword_matching_modes() {
        let keywords = vec!["báo giá".to_string(), "tư vấn".to_string()];
        assert!(matches_comment("Cho mình xin báo giá với", &keywords, MatchMode::Contains));
        assert!(!matches_comment("Sản phẩm đẹp quá", &keywords, MatchMode::Contains));
        assert!(matches_comment("Bất kỳ nội dung nào", &[], MatchMode::All));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_keyword_matching_modes`  
Expected: FAIL.

- [ ] **Step 3: Implement Automation Types & Matcher**

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

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_keyword_matching_modes`  
Expected: PASS.

- [ ] **Step 5: Commit Task 1**

```bash
git add src-tauri/src/conversation/
git commit -m "feat(facebook): add comment automation schema, types, and keyword matcher"
```

---

### Task 2: Comment Automation Engine with Loop & 7-Day Guards

**Files:**
- Create: `src-tauri/src/conversation/facebook/comment_engine.rs`
- Test: `src-tauri/src/conversation/facebook/comment_engine_test.rs`

**Interfaces:**
- Produces:
  - `CommentEngine::process_comment(comment: FacebookCommentEvent, local_db: &Connection, client: &FacebookClient) -> Result<CommentExecutionResult>`

- [ ] **Step 1: Write test for Loop Guard & 7-Day Window Filter**

```rust
// src-tauri/src/conversation/facebook/comment_engine_test.rs
#[cfg(test)]
mod tests {
    use super::super::comment_engine::*;

    #[test]
    fn test_loop_guard_rejection() {
        let page_id = "page_123";
        let sender_id = "page_123"; // Sender is the page itself
        assert!(is_self_loop(page_id, sender_id));

        let customer_id = "user_456";
        assert!(!is_self_loop(page_id, customer_id));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_loop_guard_rejection`  
Expected: FAIL.

- [ ] **Step 3: Implement `CommentEngine`**

```rust
// src-tauri/src/conversation/facebook/comment_engine.rs
use chrono::{DateTime, Duration, Utc};

pub fn is_self_loop(page_id: &str, sender_id: &str) -> bool {
    page_id == sender_id
}

pub fn is_outside_7_days(comment_created_time: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    now - comment_created_time > Duration::days(7)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_loop_guard_rejection`  
Expected: PASS.

- [ ] **Step 5: Commit Task 2**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(facebook): implement comment automation engine with loop and 7-day guards"
```

---

### Task 3: Facebook Lead Ads Ingestion Pipeline

**Files:**
- Create: `src-tauri/src/conversation/facebook/lead_ads_ingestion.rs`
- Test: `src-tauri/src/conversation/facebook/lead_ads_test.rs`

**Interfaces:**
- Produces:
  - `LeadAdsIngestor::fetch_and_ingest_lead(leadgen_id: &str, page_token: &str, local_db: &Connection) -> Result<i64>`

- [ ] **Step 1: Write test for Lead Ads Field Parsing**

```rust
// src-tauri/src/conversation/facebook/lead_ads_test.rs
#[cfg(test)]
mod tests {
    use super::super::lead_ads_ingestion::*;
    use serde_json::json;

    #[test]
    fn test_parse_lead_fields() {
        let field_data = json!([
            { "name": "full_name", "values": ["Nguyễn Văn A"] },
            { "name": "phone_number", "values": ["+84987654321"] },
            { "name": "email", "values": ["nva@example.com"] }
        ]);

        let parsed = extract_lead_info(&field_data);
        assert_eq!(parsed.name, "Nguyễn Văn A");
        assert_eq!(parsed.phone, "+84987654321");
        assert_eq!(parsed.email, "nva@example.com");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_parse_lead_fields`  
Expected: FAIL.

- [ ] **Step 3: Implement Lead Ads Ingestor**

```rust
// src-tauri/src/conversation/facebook/lead_ads_ingestion.rs
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_parse_lead_fields`  
Expected: PASS.

- [ ] **Step 5: Commit Task 3**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(facebook): implement lead ads ingestion parser and CRM pipeline"
```

---

### Task 4: Data Deletion Callback & Token Health Monitor

**Files:**
- Create: `supabase/functions/meta-data-deletion/index.ts`
- Create: `src-tauri/src/conversation/facebook/health_monitor.rs`

**Interfaces:**
- Produces:
  - Supabase Edge Function `/meta-data-deletion` (parses signed_request, deletes user data, returns confirmation_code & url)
  - Rust `PageHealthMonitor::check_pages_health(local_db: &Connection) -> Vec<PageHealthStatus>`

- [ ] **Step 1: Implement `/meta-data-deletion` Edge Function**
- [ ] **Step 2: Implement Page Health Monitor in Rust**
- [ ] **Step 3: Commit Task 4**

```bash
git add supabase/functions/ src-tauri/src/conversation/facebook/
git commit -m "feat(compliance): implement data deletion callback and page health monitor"
```

---

### Task 5: Tauri IPC Commands for Automations & Lead Ads

**Files:**
- Modify: `src-tauri/src/conversation/facebook/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces Tauri IPC commands:
  - `list_comment_automations(page_id: String) -> Result<Vec<AutomationRuleDto>>`
  - `save_comment_automation(rule: SaveAutomationRuleDto) -> Result<()>`
  - `delete_comment_automation(rule_id: i64) -> Result<()>`
  - `list_comment_audit_logs(page_id: String, limit: u32) -> Result<Vec<CommentLogDto>>`
  - `get_facebook_health_summary() -> Result<FacebookHealthSummaryDto>`

- [ ] **Step 1: Implement commands and register in Tauri generate_handler**
- [ ] **Step 2: Run cargo test**
- [ ] **Step 3: Commit Task 5**

```bash
git add src-tauri/
git commit -m "feat(tauri): add comment automation and health monitor IPC commands"
```

---

### Task 6: Svelte 5 Comment Automation UI & Audit Logs

**Files:**
- Create: `src/lib/components/facebook/CommentAutomation.svelte`
- Create: `src/lib/components/facebook/RuleBuilderDialog.svelte`
- Create: `src/lib/components/facebook/AutomationAuditLogs.svelte`

**Interfaces:**
- Produces:
  - Rule management table (Enable/Disable toggles, keyword pills).
  - Modal Rule Builder (Auto-like, Public Reply template, Private Reply template with 1-reply warning).
  - Real-time Audit Logs with filter by status (`Success`, `Skipped`, `Failed`) and explicit skip reasons.

- [ ] **Step 1: Implement Svelte 5 components**
- [ ] **Step 2: Run `bun run check`**
- [ ] **Step 3: Commit Task 6**

```bash
git add src/lib/components/facebook/
git commit -m "feat(ui): implement CommentAutomation, RuleBuilder, and AutomationAuditLogs"
```

---

### Task 7: Svelte 5 Facebook Hub Tab & Global Navigation

**Files:**
- Create: `src/lib/components/facebook/FacebookHub.svelte`
- Create: `src/lib/components/facebook/PageHealthBadge.svelte`
- Modify: `src/lib/components/Sidebar.svelte`
- Modify: `src/routes/+page.svelte`

**Interfaces:**
- Produces:
  - Facebook Hub top-level navigation tab with Lucide icon.
  - Sub-tabs: `Quản lý Trang (Pages)`, `Lên lịch bài viết (Planner)`, `Tự động hoá (Automations)`.
  - Global `PageHealthBadge` indicating live webhook status and token validity.

- [ ] **Step 1: Add Facebook navigation item to `Sidebar.svelte`**
- [ ] **Step 2: Mount `FacebookHub.svelte` in `+page.svelte`**
- [ ] **Step 3: Run `bun run check`**
- [ ] **Step 4: Commit Task 7**

```bash
git add src/lib/components/ src/routes/
git commit -m "feat(ui): add FacebookHub top-level view and sidebar navigation"
```

---

### Task 8: End-to-End Test Suite & App Review Preparation Package

**Files:**
- Test: Full integration test suite & App review screencast checklist

- [ ] **Step 1: Run complete Rust test suite**  
  `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] **Step 2: Run frontend build and linter**  
  `bun run build && bun run check`
- [ ] **Step 3: Final Commit for Phase 3**

```bash
git commit --allow-empty -m "chore: complete phase 3 facebook automations and compliance implementation"
```
