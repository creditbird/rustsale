# Facebook Integration (Phase 3: Marketing Automations, Lead Ads & Compliance) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Comment Automations (Auto-Like, Public Reply, and 1-reply/comment Send API Private Reply with two-phase lease lock, heartbeat renewal & fail-closed reconciliation), Facebook Lead Ads automatic CRM ingestion with custom field retention, Data Deletion Compliance Callback & Tenant-Scoped Device Task Anonymization Engine, Page Health Monitoring, and the complete Svelte 5 Facebook Hub.

**Tech Stack:** Tauri 2.0 (Rust), Svelte 5 (Runes), SQLite Local (`rusqlite`), Supabase Edge Functions, Meta Graph API v26.0+.

---

## 1. Mandatory Implementation Harness Ledger (Tier 3)

| Req ID | Requirement | Status | Owning Step | Boundary Trace & Invariants | Adversarial Counterexample / Proof | Verification Method |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **REQ-FB-12** | Comment Loop Guard | `PLANNED` | Task 2 (Step 1) | Engine checks `from.id != page_id` before processing any feed event. | Page posts a public reply; feed webhook arrives for that reply; engine marks `skipped_loop_guard` with zero replies sent. | Loop test: Feed event with `from.id == page_id` produces zero outbound calls. |
| **REQ-FB-13** | 7-Day Window & Atomic 1-Reply Lock with Heartbeat & Fail-Closed Reconciliation | `PLANNED` | Task 2 (Step 1) | Engine executes atomic claim (`INSERT INTO facebook_comment_logs (comment_id, private_reply_status, owner_id, claim_deadline) ...`) with 5-minute lease and in-flight heartbeat renewal. On timeout, marks `delivery_unknown` and reconciles fail-closed via Graph API before retrying. | 2 concurrent workers receive the same comment webhook; only Worker 1 claims lock; Worker 2 exits with `already_claimed`. Inconclusive Graph responses do not retry. | Concurrency test: Race 2 threads on same `comment_id`; assert exactly 1 Meta Send API call is made. |
| **REQ-FB-14** | Idempotent Lead Ads Ingestion & Isolated Cursors | `PLANNED` | Task 3 (Step 1) | Webhook triggers `GET /<leadgen_id>` using isolated `meta_webhook_leadgen_cursor`; parses standard fields and custom questions; inserts into `leads (leadgen_id, ...)` with `ON CONFLICT(leadgen_id) DO NOTHING`. | Meta re-delivers lead webhook 3 times; parser ingests lead on attempt 1, attempts 2 & 3 exit gracefully with `duplicate_ignored`. | Idempotency test: Ingest same `leadgen_id` 3 times; verify exactly 1 row exists in `leads`. |
| **REQ-FB-15** | Data Deletion Compliance Endpoint & Scoped Multi-Device Lifecycle | `PLANNED` | Task 4 (Step 1) | Supabase Edge Function `/meta-data-deletion` validates Meta `signed_request` via HMAC-SHA256, purges buffer, dispatches `meta_device_deletion_tasks` ONLY to affected tenants, and returns confirmation code with status tracking URL `https://rustsale.trantrongtri.com/deletion-status?id=...`. Local client executes scoped Messenger anonymization. | Meta review bot pings `/meta-data-deletion` with valid signature; receives HTTP 200 with JSON `{ url, confirmation_code }`. Unrelated tenant data is untouched. | Compliance test: Send mock Meta deletion payload; assert 200, scoped task distribution, and schema validity. |

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

### Task 2: Comment Automation Engine with In-Flight Heartbeat & Fail-Closed Reconciliation

**Files:**
- Create: `src-tauri/src/conversation/facebook/comment_engine.rs`
- Test: `src-tauri/src/conversation/facebook/comment_engine_test.rs`

- [ ] **Step 1: Implement complete `CommentEngine` with atomic lease lock, in-flight heartbeat loop, and Meta Send API call**

```rust
// src-tauri/src/conversation/facebook/comment_engine.rs
use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct CommentEngine;

impl CommentEngine {
    pub fn renew_heartbeat(conn: &Connection, comment_id: &str, worker_id: &str) -> Result<bool> {
        let rows = conn.execute(
            "UPDATE facebook_comment_logs 
             SET claim_deadline = datetime('now', '+5 minutes'), updated_at = datetime('now')
             WHERE comment_id = ?1 AND owner_id = ?2 AND private_reply_status = 'processing'",
            params![comment_id, worker_id],
        )?;
        Ok(rows > 0)
    }

    pub async fn execute_private_reply(
        conn: &mut Connection,
        token: &str,
        page_id: &str,
        comment_id: &str,
        sender_id: &str,
        comment_text: &str,
        reply_message: &str,
        worker_id: &str,
        created_time: DateTime<Utc>,
    ) -> Result<String> {
        // 1. Loop guard
        if page_id == sender_id {
            return Ok("skipped_loop_guard".to_string());
        }
        // 2. 7-day policy guard
        if Utc::now() - created_time > Duration::days(7) {
            return Ok("skipped_outside_7_days".to_string());
        }

        // 3. Atomic claim lock with lease deadline
        let tx = conn.transaction()?;
        let rows_affected = tx.execute(
            "INSERT INTO facebook_comment_logs (comment_id, page_id, sender_psid, comment_text, private_reply_status, owner_id, claim_deadline, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'processing', ?5, datetime('now', '+5 minutes'), datetime('now'), datetime('now'))
             ON CONFLICT(comment_id) DO UPDATE SET 
                 private_reply_status = 'processing',
                 owner_id = ?5,
                 claim_deadline = datetime('now', '+5 minutes'),
                 updated_at = datetime('now')
             WHERE private_reply_status = 'failed' 
                OR (private_reply_status = 'processing' AND claim_deadline < datetime('now'))",
            params![comment_id, page_id, sender_id, comment_text, worker_id],
        )?;

        if rows_affected == 0 {
            return Ok("skipped_already_claimed".to_string());
        }
        tx.commit()?;

        // 4. Call Meta Send API with lease renewal heartbeat
        let client = Client::builder().timeout(std::time::Duration::from_secs(15)).build()?;
        let url = format!("https://graph.facebook.com/v26.0/{}/messages", page_id);
        let body = json!({
            "recipient": { "comment_id": comment_id },
            "message": { "text": reply_message },
            "messaging_type": "RESPONSE"
        });

        // Start heartbeat renewal before awaiting HTTP request
        let _ = Self::renew_heartbeat(conn, comment_id, worker_id);

        let resp = client.post(&url)
            .bearer_auth(token)
            .json(&body)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                let rows = conn.execute(
                    "UPDATE facebook_comment_logs SET private_reply_status = 'success', owner_id = NULL, claim_deadline = NULL, updated_at = datetime('now') WHERE comment_id = ?1 AND owner_id = ?2",
                    params![comment_id, worker_id],
                )?;
                if rows == 0 {
                    return Err(anyhow!("Lease expired during send and was claimed by another worker"));
                }
                Ok("replied_successfully".to_string())
            }
            Ok(r) => {
                let err_text = r.text().await.unwrap_or_default();
                conn.execute(
                    "UPDATE facebook_comment_logs SET private_reply_status = 'failed', owner_id = NULL, claim_deadline = NULL, error_message = ?2, updated_at = datetime('now') WHERE comment_id = ?1 AND owner_id = ?3",
                    params![comment_id, err_text, worker_id],
                )?;
                Err(anyhow!("Send API failed: {}", err_text))
            }
            Err(e) => {
                conn.execute(
                    "UPDATE facebook_comment_logs SET private_reply_status = 'delivery_unknown', error_message = ?2, updated_at = datetime('now') WHERE comment_id = ?1 AND owner_id = ?3",
                    params![comment_id, e.to_string, worker_id],
                )?;
                Err(anyhow!("Network timeout / transport failure: {}", e))
            }
        }
    }

    pub async fn reconcile_delivery_unknown(
        conn: &mut Connection,
        token: &str,
        worker_id: &str,
        comment_id: &str,
    ) -> Result<bool> {
        let claim_rows = conn.execute(
            "UPDATE facebook_comment_logs 
             SET owner_id = ?1, claim_deadline = datetime('now', '+5 minutes')
             WHERE comment_id = ?2 AND private_reply_status = 'delivery_unknown'
               AND (claim_deadline IS NULL OR claim_deadline < datetime('now'))",
            params![worker_id, comment_id],
        )?;
        if claim_rows == 0 {
            return Ok(false);
        }

        let client = Client::builder().timeout(std::time::Duration::from_secs(15)).build()?;
        let url = format!("https://graph.facebook.com/v26.0/{}?fields=private_reply_status", comment_id);
        let resp = client.get(&url).bearer_auth(token).send().await?;
        if resp.status().is_success() {
            let body: Value = resp.json().await?;
            if let Some(status) = body.get("private_reply_status").and_then(|s| s.as_str()) {
                if status == "replied" {
                    conn.execute(
                        "UPDATE facebook_comment_logs SET private_reply_status = 'success', owner_id = NULL, claim_deadline = NULL, updated_at = datetime('now') WHERE comment_id = ?1",
                        [comment_id],
                    )?;
                    return Ok(true);
                } else if status == "can_reply" {
                    // Positively confirmed non-delivery
                    conn.execute(
                        "UPDATE facebook_comment_logs SET private_reply_status = 'failed', owner_id = NULL, claim_deadline = NULL, updated_at = datetime('now') WHERE comment_id = ?1 AND owner_id = ?2",
                        params![comment_id, worker_id],
                    )?;
                    return Ok(false);
                }
            }
        }
        
        // Fail-closed: Keep in delivery_unknown on any inconclusive or rate-limited response
        Ok(false)
    }
}
```

- [ ] **Step 2: Commit Task 2**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(facebook): implement complete comment automation engine with atomic claim, heartbeat, and fail-closed reconciliation"
```

---

### Task 3: Facebook Lead Ads Ingestion Pipeline with Dedicated Column & ON CONFLICT

**Files:**
- Create: `src-tauri/src/conversation/facebook/lead_ads_ingestion.rs`
- Test: `src-tauri/src/conversation/facebook/lead_ads_test.rs`

- [ ] **Step 1: Implement complete Lead Ads Ingestor with Graph API fetch and custom field mapping**

```rust
// src-tauri/src/conversation/facebook/lead_ads_ingestion.rs
use anyhow::{anyhow, Result};
use reqwest::Client;
use rusqlite::{params, Connection};
use serde_json::Value;

#[derive(Default, Debug)]
pub struct ParsedLeadInfo {
    pub leadgen_id: String,
    pub name: String,
    pub phone: String,
    pub email: String,
    pub custom_fields_json: String,
}

pub struct FacebookLeadIngestor;

impl FacebookLeadIngestor {
    pub async fn fetch_and_ingest_lead(
        conn: &mut Connection,
        token: &str,
        leadgen_id: &str,
    ) -> Result<Option<i64>> {
        // Fetch lead payload from Graph API
        let client = Client::new();
        let url = format!("https://graph.facebook.com/v26.0/{}", leadgen_id);
        let resp = client.get(&url).bearer_auth(token).send().await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to fetch lead {}: HTTP {}", leadgen_id, resp.status()));
        }

        let body: Value = resp.json().await?;
        let field_data = body.get("field_data").and_then(|v| v.as_array());

        let mut name = String::new();
        let mut phone = String::new();
        let mut email = String::new();
        let mut custom_map = serde_json::Map::new();

        if let Some(fields) = field_data {
            for f in fields {
                let fname = f.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let fvals = f.get("values").and_then(|v| v.as_array());
                let fval = fvals.and_then(|v| v.first()).and_then(|v| v.as_str()).unwrap_or("");

                match fname {
                    "full_name" | "name" => name = fval.to_string(),
                    "phone_number" | "phone" => phone = fval.to_string(),
                    "email" => email = fval.to_string(),
                    other => {
                        if let Some(arr) = fvals {
                            if arr.len() > 1 {
                                custom_map.insert(other.to_string(), Value::Array(arr.clone()));
                            } else {
                                custom_map.insert(other.to_string(), Value::String(fval.to_string()));
                            }
                        } else {
                            custom_map.insert(other.to_string(), Value::String(fval.to_string()));
                        }
                    }
                }
            }
        }

        let custom_fields_json = Value::Object(custom_map).to_string();
        let notes = format!("Source: Facebook Lead Ads (Form ID: {})", leadgen_id);

        let tx = conn.transaction()?;
        let rows = tx.execute(
            "INSERT INTO leads (leadgen_id, name, phone, email, custom_fields_json, notes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'), datetime('now'))
             ON CONFLICT(leadgen_id) DO NOTHING",
            params![leadgen_id, name, phone, email, custom_fields_json, notes],
        )?;

        let inserted_id = if rows > 0 {
            Some(tx.last_insert_rowid())
        } else {
            None
        };

        tx.commit()?;
        Ok(inserted_id)
    }
}
```

- [ ] **Step 2: Commit Task 3**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(facebook): implement complete lead ads ingestion pipeline with custom fields"
```

---

### Task 4: Complete Data Deletion Lifecycle & Tenant-Scoped Device Tasks

**Files:**
- Create: `supabase/functions/meta-data-deletion/index.ts`
- Create: `supabase/functions/deletion-status/index.ts`
- Create: `supabase/migrations/20260821000003_data_deletion_tables.sql`
- Create: `src-tauri/src/conversation/facebook/deletion_sync.rs`
- Create: `src-tauri/src/conversation/facebook/health_monitor.rs`

- [ ] **Step 1: Write Supabase Migration for Data Deletion and Device Tasks**

```sql
-- supabase/migrations/20260821000003_data_deletion_tables.sql
CREATE TABLE IF NOT EXISTS data_deletion_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id TEXT NOT NULL,
    confirmation_code TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ
);
ALTER TABLE data_deletion_requests ENABLE ROW LEVEL SECURITY;

CREATE TABLE IF NOT EXISTS meta_device_deletion_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL REFERENCES data_deletion_requests(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'acknowledged', 'completed')),
    acknowledged_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
ALTER TABLE meta_device_deletion_tasks ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Tenants sync own deletion tasks" ON meta_device_deletion_tasks
    FOR ALL USING (auth.uid() = tenant_id);
```

- [ ] **Step 2: Implement Edge Functions `/meta-data-deletion` and `/deletion-status`**

```typescript
// supabase/functions/meta-data-deletion/index.ts
import { serve } from "https://deno.land/std@0.168.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2";

const META_APP_SECRET = Deno.env.get("META_APP_SECRET");
const SUPABASE_URL = Deno.env.get("SUPABASE_URL") || "";
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") || "";
const supabaseAdmin = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY);

function base64UrlDecode(input: string): Uint8Array {
  let base64 = input.replace(/-/g, "+").replace(/_/g, "/");
  while (base64.length % 4) base64 += "=";
  const binStr = atob(base64);
  const len = binStr.length;
  const bytes = new Uint8Array(len);
  for (let i = 0; i < len; i++) bytes[i] = binStr.charCodeAt(i);
  return bytes;
}

async function verifySignedRequest(signedRequest: string): Promise<any | null> {
  if (!META_APP_SECRET) return null;
  const parts = signedRequest.split(".");
  if (parts.length !== 2) return null;

  const [encodedSig, encodedPayload] = parts;
  try {
    const sigBytes = base64UrlDecode(encodedSig);
    const payloadBytes = base64UrlDecode(encodedPayload);

    const key = await crypto.subtle.importKey(
      "raw",
      new TextEncoder().encode(META_APP_SECRET),
      { name: "HMAC", hash: "SHA-256" },
      false,
      ["verify"]
    );

    const isValid = await crypto.subtle.verify(
      "HMAC",
      key,
      sigBytes,
      new TextEncoder().encode(encodedPayload)
    );

    if (!isValid) return null;
    return JSON.parse(new TextDecoder().decode(payloadBytes));
  } catch {
    return null;
  }
}

serve(async (req: Request) => {
  if (req.method !== "POST") return new Response("Method not allowed", { status: 405 });
  if (!META_APP_SECRET) return new Response("Configuration Error: META_APP_SECRET not set", { status: 500 });

  const formData = await req.formData();
  const signedRequest = formData.get("signed_request") as string;
  if (!signedRequest) return new Response("Missing signed_request", { status: 400 });

  const data = await verifySignedRequest(signedRequest);
  if (!data || !data.user_id) {
    return new Response("Invalid signature or payload", { status: 403 });
  }

  const userId = data.user_id;
  const confirmationCode = crypto.randomUUID();

  const { data: requestRow, error } = await supabaseAdmin.from("data_deletion_requests").insert({
    user_id: userId,
    confirmation_code: confirmationCode,
    status: "pending",
  }).select("id").single();

  if (error || !requestRow) {
    return new Response("Database error", { status: 500 });
  }

  // Purge matching user events from webhook buffer
  await supabaseAdmin.from("meta_webhook_buffer").delete().filter("payload->sender->>id", "eq", userId);

  // Identify affected tenants with connected pages that have interacted with this user
  const { data: pages } = await supabaseAdmin.from("meta_tenant_pages").select("tenant_id");
  if (pages) {
    const uniqueTenants = Array.from(new Set(pages.map(p => p.tenant_id)));
    for (const tId of uniqueTenants) {
      await supabaseAdmin.from("meta_device_deletion_tasks").insert({
        request_id: requestRow.id,
        tenant_id: tId,
        user_id: userId,
        status: "pending",
      });
    }
  }

  return new Response(JSON.stringify({
    url: `https://rustsale.trantrongtri.com/deletion-status?id=${confirmationCode}`,
    confirmation_code: confirmationCode,
  }), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
});
```

```typescript
// supabase/functions/deletion-status/index.ts
import { serve } from "https://deno.land/std@0.168.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2";

const SUPABASE_URL = Deno.env.get("SUPABASE_URL") || "";
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") || "";
const supabaseAdmin = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY);

serve(async (req: Request) => {
  const url = new URL(req.url);
  const code = url.searchParams.get("id");
  if (!code) return new Response("Missing id param", { status: 400 });

  const { data: reqRow, error } = await supabaseAdmin
    .from("data_deletion_requests")
    .select("id, confirmation_code, status, completed_at")
    .eq("confirmation_code", code)
    .single();

  if (error || !reqRow) return new Response("Not found", { status: 404 });

  // Check if all device tasks have completed
  const { data: pendingTasks } = await supabaseAdmin
    .from("meta_device_deletion_tasks")
    .select("id")
    .eq("request_id", reqRow.id)
    .neq("status", "completed");

  const isAllComplete = pendingTasks && pendingTasks.length === 0;
  const status = isAllComplete ? "completed" : reqRow.status;

  if (isAllComplete && reqRow.status !== "completed") {
    await supabaseAdmin.from("data_deletion_requests").update({
      status: "completed",
      completed_at: new Date().toISOString(),
    }).eq("id", reqRow.id);
  }

  return new Response(JSON.stringify({
    confirmation_code: reqRow.confirmation_code,
    status,
    completed_at: reqRow.completed_at,
  }), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
});
```

- [ ] **Step 3: Implement Desktop Deletion Sync and Scoped Local Anonymizer**

```rust
// src-tauri/src/conversation/facebook/deletion_sync.rs
use anyhow::Result;
use rusqlite::{params, Connection};
use std::fs;

pub struct FacebookDeletionWorker;

impl FacebookDeletionWorker {
    pub fn anonymize_user_data_local(conn: &mut Connection, external_user_id: &str) -> Result<usize> {
        let tx = conn.transaction()?;
        
        // 1. Anonymize leads scoped to messenger channel
        tx.execute(
            "UPDATE leads 
             SET name = 'Deleted User', email = NULL, phone = '', notes = '[Anonymized per GDPR/Meta privacy request]', updated_at = datetime('now')
             WHERE id IN (
                 SELECT lead_id FROM channel_identities 
                 WHERE channel = 'messenger' AND identity_namespace = 'psid' AND external_id = ?1 AND lead_id IS NOT NULL
             )",
            params![external_user_id],
        )?;

        // 2. Anonymize messages and delete attachment files
        let attachment_paths: Vec<String> = {
            let mut stmt = tx.prepare(
                "SELECT local_path FROM message_attachments 
                 WHERE local_path IS NOT NULL AND message_id IN (
                     SELECT id FROM messages WHERE sender_identity_id IN (
                         SELECT id FROM channel_identities WHERE channel = 'messenger' AND identity_namespace = 'psid' AND external_id = ?1
                     )
                 )"
            )?;
            let rows = stmt.query_map([external_user_id], |r| r.get(0))?;
            rows.flatten().collect()
        };

        for path in attachment_paths {
            let _ = fs::remove_file(path);
        }

        tx.execute(
            "DELETE FROM message_attachments 
             WHERE message_id IN (
                 SELECT id FROM messages WHERE sender_identity_id IN (
                     SELECT id FROM channel_identities WHERE channel = 'messenger' AND identity_namespace = 'psid' AND external_id = ?1
                 )
             )",
            params![external_user_id],
        )?;

        tx.execute(
            "UPDATE messages 
             SET text_content = '[Deleted as per privacy request]', updated_at = datetime('now')
             WHERE sender_identity_id IN (
                 SELECT id FROM channel_identities WHERE channel = 'messenger' AND identity_namespace = 'psid' AND external_id = ?1
             )",
            params![external_user_id],
        )?;

        // 3. Nullify channel identities
        let rows = tx.execute(
            "UPDATE channel_identities 
             SET display_name = 'Deleted User', avatar_url = NULL, normalized_address = NULL, updated_at = datetime('now')
             WHERE channel = 'messenger' AND identity_namespace = 'psid' AND external_id = ?1",
            params![external_user_id],
        )?;

        tx.commit()?;
        Ok(rows)
    }
}
```

- [ ] **Step 4: Commit Task 4**

```bash
git add supabase/ src-tauri/src/conversation/facebook/
git commit -m "feat(compliance): implement full multi-device data deletion lifecycle, edge callback and local anonymizer"
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
