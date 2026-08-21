# Facebook Integration (Phase 3: Marketing Automations, Lead Ads & Compliance) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Comment Automations (Auto-Like, Public Reply, and 1-reply/comment Send API Private Reply with two-phase lease lock, in-flight heartbeat renewal loop & fail-closed reconciliation), Facebook Lead Ads automatic CRM ingestion with custom field retention & isolated cursor worker, Data Deletion Compliance Callback with Durable Interaction Mapping, Tenant/Page Scoped Device Task Distribution & Transactional Anonymization Engine via `pending_spool_cleanups`, Page Health Monitoring, and the complete Svelte 5 Facebook Hub.

**Tech Stack:** Tauri 2.0 (Rust), Svelte 5 (Runes), SQLite Local (`rusqlite`), Supabase Edge Functions, Meta Graph API v26.0+.

---

## 1. Mandatory Implementation Harness Ledger (Tier 3)

| Req ID | Requirement | Status | Owning Step | Boundary Trace & Invariants | Adversarial Counterexample / Proof | Verification Method |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **REQ-FB-12** | Comment Loop Guard | `PLANNED` | Task 2 (Step 1) | Engine checks `from.id != page_id` before processing any feed event. | Page posts a public reply; feed webhook arrives for that reply; engine marks `skipped_loop_guard` with zero replies sent. | Loop test: Feed event with `from.id == page_id` produces zero outbound calls. |
| **REQ-FB-13** | 7-Day Window & Atomic 1-Reply Lock with Active Heartbeat & Fail-Closed Reconciliation | `PLANNED` | Task 2 (Step 1) | Engine executes atomic claim (`INSERT INTO facebook_comment_logs (comment_id, private_reply_status, owner_id, claim_deadline) ...`) with 5-minute lease and in-flight heartbeat renewal channel. On timeout or lease abort, marks `delivery_unknown` and reconciles fail-closed via Graph API before retrying. | 2 concurrent workers receive the same comment webhook; only Worker 1 claims lock; Worker 2 exits with `already_claimed`. Inconclusive Graph responses do not retry. | Concurrency test: Race 2 threads on same `comment_id`; assert exactly 1 Meta Send API call is made. |
| **REQ-FB-14** | Idempotent Lead Ads Ingestion & Isolated Cursor Worker | `PLANNED` | Task 3 (Step 1) | Webhook triggers worker reading `meta_webhook_buffer` via `meta_webhook_leadgen_cursor`; fetches `GET /<leadgen_id>`; parses standard fields and custom questions; inserts into `leads (leadgen_id, ...)` with `ON CONFLICT(leadgen_id) WHERE leadgen_id IS NOT NULL DO NOTHING`. Quarantines failed lead fetches. | Meta re-delivers lead webhook 3 times; parser ingests lead on attempt 1, attempts 2 & 3 exit gracefully with `duplicate_ignored`. | Idempotency test: Ingest same `leadgen_id` 3 times; verify exactly 1 row exists in `leads`. |
| **REQ-FB-15** | Data Deletion Compliance Endpoint & Scoped Multi-Device Lifecycle | `PLANNED` | Task 4 (Step 1) | Supabase Edge Function `/meta-data-deletion` validates Meta `signed_request` via HMAC-SHA256, executes transactional PostgreSQL RPC creating `data_deletion_requests` and `meta_device_deletion_tasks` scoped to Page and tenant, and returns confirmation code with status tracking URL. Local client executes comprehensive anonymization inventory, sweeps `pending_spool_cleanups`, and acknowledges remote task completion. | Meta review bot pings `/meta-data-deletion` with valid signature; receives HTTP 200 with JSON `{ url, confirmation_code }`. Unrelated tenant data is untouched. | Compliance test: Send mock Meta deletion payload; assert 200, scoped task distribution, and schema validity. |

---

### Task 1: SQLite Schema & Types for Comment Automations

**Files:**
- Modify: `src-tauri/src/conversation/schema.sql`
- Create: `src-tauri/src/conversation/facebook/automation_types.rs`
- Test: `src-tauri/src/conversation/facebook/automation_types_test.rs`

- [ ] **Step 1: Implement Automation Types with Regex and keyword matcher**

```rust
// src-tauri/src/conversation/facebook/automation_types.rs
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchMode {
    Contains,
    Exact,
    Regex,
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
        MatchMode::Regex => {
            keywords.iter().any(|pattern| {
                if pattern.len() > 500 {
                    return false;
                }
                match RegexBuilder::new(pattern).case_insensitive(true).size_limit(1024 * 1024).build() {
                    Ok(re) => re.is_match(comment_text),
                    Err(_) => false,
                }
            })
        }
        MatchMode::All => true,
    }
}
```

- [ ] **Step 2: Commit Task 1**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(facebook): implement comment automation types with regex support and keyword matcher"
```

---

### Task 2: Comment Automation Engine with Active Heartbeat Loop & Fail-Closed Reconciliation

**Files:**
- Create: `src-tauri/src/conversation/facebook/comment_engine.rs`
- Test: `src-tauri/src/conversation/facebook/comment_engine_test.rs`

- [ ] **Step 1: Implement complete `CommentEngine` with atomic lease lock, active heartbeat channel, and Meta Send API call**

```rust
// src-tauri/src/conversation/facebook/comment_engine.rs
use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

pub struct CommentEngine;

impl CommentEngine {
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

        // 3. Atomic claim lock with 5-minute lease deadline
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

        // 4. Call Meta Send API with 15s timeout
        let client = Client::builder().timeout(std::time::Duration::from_secs(15)).build()?;
        let url = format!("https://graph.facebook.com/v26.0/{}/messages", page_id);
        let body = json!({
            "recipient": { "comment_id": comment_id },
            "message": { "text": reply_message },
            "messaging_type": "RESPONSE"
        });

        let resp = client.post(&url)
            .bearer_auth(token)
            .json(&body)
            .send().await;

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
                let status_code = r.status();
                let err_text = r.text().await.unwrap_or_default();
                if status_code.is_server_error() {
                    conn.execute(
                        "UPDATE facebook_comment_logs SET private_reply_status = 'delivery_unknown', owner_id = NULL, claim_deadline = NULL, error_message = ?2, updated_at = datetime('now') WHERE comment_id = ?1 AND owner_id = ?3",
                        params![comment_id, err_text, worker_id],
                    )?;
                } else {
                    conn.execute(
                        "UPDATE facebook_comment_logs SET private_reply_status = 'failed', owner_id = NULL, claim_deadline = NULL, error_message = ?2, updated_at = datetime('now') WHERE comment_id = ?1 AND owner_id = ?3",
                        params![comment_id, err_text, worker_id],
                    )?;
                }
                Err(anyhow!("Send API failed: {}", err_text))
            }
            Err(e) => {
                conn.execute(
                    "UPDATE facebook_comment_logs SET private_reply_status = 'delivery_unknown', owner_id = NULL, claim_deadline = NULL, error_message = ?2, updated_at = datetime('now') WHERE comment_id = ?1 AND owner_id = ?3",
                    params![comment_id, e.to_string(), worker_id],
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
                        "UPDATE facebook_comment_logs SET private_reply_status = 'success', owner_id = NULL, claim_deadline = NULL, updated_at = datetime('now') WHERE comment_id = ?1 AND owner_id = ?2",
                        params![comment_id, worker_id],
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
git commit -m "feat(facebook): implement complete comment automation engine with active heartbeat channel and fail-closed reconciliation"
```

---

### Task 3: Facebook Lead Ads Ingestion Pipeline with Isolated Cursor Worker

**Files:**
- Create: `src-tauri/src/conversation/facebook/lead_ads_ingestion.rs`
- Test: `src-tauri/src/conversation/facebook/lead_ads_test.rs`

- [ ] **Step 1: Implement complete Lead Ads Ingestor and Cursor Worker with Failure Quarantine**

```rust
// src-tauri/src/conversation/facebook/lead_ads_ingestion.rs
use anyhow::{anyhow, Result};
use reqwest::Client;
use rusqlite::{params, Connection};
use serde_json::Value;

pub struct FacebookLeadIngestor;

impl FacebookLeadIngestor {
    pub async fn fetch_and_ingest_lead(
        conn: &mut Connection,
        token: &str,
        leadgen_id: &str,
    ) -> Result<Option<i64>> {
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
             ON CONFLICT(leadgen_id) WHERE leadgen_id IS NOT NULL DO NOTHING",
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

    pub async fn process_leadgen_events(
        conn: &mut Connection,
        channel_account_id: i64,
        token: &str,
        events: &[Value],
    ) -> Result<i64> {
        let mut max_cursor = 0i64;

        for event in events {
            let event_id = event.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            let payload = event.get("payload");
            
            // Support both direct leadgen_id property and nested value.leadgen_id shape
            let leadgen_id = payload.and_then(|p| p.get("leadgen_id").or_else(|| p.get("value").and_then(|v| v.get("leadgen_id"))))
                .and_then(|l| l.as_str()).unwrap_or("");

            if leadgen_id.is_empty() {
                conn.execute(
                    "INSERT INTO channel_sync_dead_letters (channel_account_id, scope, event_id, item_index, event_type, event_key, payload_json, error_reason, status, created_at)
                     VALUES (?1, 'meta_webhook_leadgen_cursor', ?2, 0, 'leadgen', '', ?3, 'missing_leadgen_id', 'failed', datetime('now'))
                     ON CONFLICT(channel_account_id, scope, event_id, item_index) DO NOTHING",
                    params![channel_account_id, event_id, serde_json::to_string(&payload)?],
                )?;
            } else {
                match Self::fetch_and_ingest_lead(conn, token, leadgen_id).await {
                    Ok(_) => {}
                    Err(e) => {
                        conn.execute(
                            "INSERT INTO channel_sync_dead_letters (channel_account_id, scope, event_id, item_index, event_type, event_key, payload_json, error_reason, status, created_at)
                             VALUES (?1, 'meta_webhook_leadgen_cursor', ?2, 0, 'leadgen', ?3, ?4, ?5, 'pending', datetime('now'))
                             ON CONFLICT(channel_account_id, scope, event_id, item_index) DO NOTHING",
                            params![channel_account_id, event_id, leadgen_id, serde_json::to_string(&payload)?, e.to_string()],
                        )?;
                    }
                }
            }

            // Monotonically advance leadgen cursor
            conn.execute(
                "INSERT INTO channel_sync_checkpoints (channel_account_id, scope, cursor, updated_at)
                 VALUES (?1, 'meta_webhook_leadgen_cursor', ?2, datetime('now'))
                 ON CONFLICT(channel_account_id, scope) DO UPDATE SET 
                     cursor = ?2,
                     updated_at = datetime('now')
                 WHERE CAST(excluded.cursor AS INTEGER) > CAST(COALESCE(channel_sync_checkpoints.cursor, '0') AS INTEGER)",
                params![channel_account_id, event_id.to_string()],
            )?;

            if event_id > max_cursor {
                max_cursor = event_id;
            }
        }

        Ok(max_cursor)
    }
}
```

- [ ] **Step 2: Commit Task 3**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(facebook): implement complete lead ads ingestion pipeline with isolated cursor worker and failure quarantine"
```

---

### Task 4: Tenant-Scoped Data Deletion Lifecycle & Post-Commit Anonymizer via Spool Cleanups

**Files:**
- Create: `supabase/functions/meta-data-deletion/index.ts`
- Create: `supabase/functions/deletion-status/index.ts`
- Create: `supabase/migrations/20260821000003_data_deletion_tables.sql`
- Create: `src-tauri/src/conversation/facebook/deletion_sync.rs`
- Create: `src-tauri/src/conversation/facebook/health_monitor.rs`

- [ ] **Step 1: Write Supabase Migration with Transactional Deletion RPC**

```sql
-- supabase/migrations/20260821000003_data_deletion_tables.sql
CREATE TABLE IF NOT EXISTS tenant_registered_devices (
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    device_name TEXT NOT NULL DEFAULT 'desktop',
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(tenant_id, device_id)
);
ALTER TABLE tenant_registered_devices ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Tenants manage own registered devices" ON tenant_registered_devices
    FOR ALL USING (auth.uid() = tenant_id);

CREATE TABLE IF NOT EXISTS data_deletion_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id TEXT NOT NULL,
    confirmation_code TEXT NOT NULL UNIQUE,
    required_tasks_count INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ
);
ALTER TABLE data_deletion_requests ENABLE ROW LEVEL SECURITY;

CREATE TABLE IF NOT EXISTS meta_device_deletion_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL REFERENCES data_deletion_requests(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    page_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'acknowledged', 'completed')),
    acknowledged_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(request_id, tenant_id, page_id, device_id)
);
ALTER TABLE meta_device_deletion_tasks ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Tenants sync own deletion tasks" ON meta_device_deletion_tasks
    FOR ALL USING (auth.uid() = tenant_id);

-- Transactional RPC for atomic deletion request creation and multi-device task dispatch
CREATE OR REPLACE FUNCTION create_data_deletion_request_and_tasks(
    p_user_id TEXT,
    p_confirmation_code TEXT
)
RETURNS JSONB
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, auth, pg_temp
AS $$
DECLARE
    v_request_id UUID;
    v_tasks_count INTEGER := 0;
    v_row RECORD;
BEGIN
    INSERT INTO public.data_deletion_requests (user_id, confirmation_code, status, created_at)
    VALUES (p_user_id, p_confirmation_code, 'pending', now())
    RETURNING id INTO v_request_id;

    -- Purge matching user events from webhook buffer across all canonical payload shapes
    DELETE FROM public.meta_webhook_buffer 
    WHERE (payload->'sender'->>'id' = p_user_id) 
       OR (payload->'recipient'->>'id' = p_user_id)
       OR (payload->'value'->'from'->>'id' = p_user_id)
       OR (payload->'raw_change'->'value'->'from'->>'id' = p_user_id);

    -- Fan out tasks across all registered tenant devices that interacted with this user
    FOR v_row IN 
        SELECT DISTINCT p.tenant_id, p.page_id, COALESCE(d.device_id, 'primary') AS device_id
        FROM public.meta_user_page_interactions p
        LEFT JOIN public.tenant_registered_devices d ON d.tenant_id = p.tenant_id
        WHERE p.user_id = p_user_id
    LOOP
        INSERT INTO public.meta_device_deletion_tasks (request_id, tenant_id, page_id, user_id, device_id, status)
        VALUES (v_request_id, v_row.tenant_id, v_row.page_id, p_user_id, v_row.device_id, 'pending')
        ON CONFLICT (request_id, tenant_id, page_id, device_id) DO NOTHING;
        v_tasks_count := v_tasks_count + 1;
    END LOOP;

    UPDATE public.data_deletion_requests 
    SET required_tasks_count = v_tasks_count,
        status = CASE WHEN v_tasks_count = 0 THEN 'completed' ELSE 'pending' END,
        completed_at = CASE WHEN v_tasks_count = 0 THEN now() ELSE NULL END
    WHERE id = v_request_id;

    RETURN jsonb_build_object(
        'request_id', v_request_id,
        'confirmation_code', p_confirmation_code,
        'tasks_count', v_tasks_count
    );
END;
$$;

-- Strictly secure deletion RPC from public or unauthorized user invocation
REVOKE ALL ON FUNCTION create_data_deletion_request_and_tasks FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION create_data_deletion_request_and_tasks TO service_role;
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

  // Execute transactional deletion RPC
  const { data: res, error } = await supabaseAdmin.rpc("create_data_deletion_request_and_tasks", {
    p_user_id: userId,
    p_confirmation_code: confirmationCode,
  });

  if (error || !res) {
    return new Response("Database error processing deletion request", { status: 500 });
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
    .select("id, confirmation_code, status, required_tasks_count, completed_at")
    .eq("confirmation_code", code)
    .single();

  if (error || !reqRow) return new Response("Not found", { status: 404 });

  let completedAt = reqRow.completed_at;

  if (reqRow.required_tasks_count > 0 && reqRow.status !== "completed") {
    const { data: completedTasks } = await supabaseAdmin
      .from("meta_device_deletion_tasks")
      .select("id")
      .eq("request_id", reqRow.id)
      .eq("status", "completed");

    const completedCount = completedTasks ? completedTasks.length : 0;
    if (completedCount >= reqRow.required_tasks_count) {
      completedAt = new Date().toISOString();
      await supabaseAdmin.from("data_deletion_requests").update({
        status: "completed",
        completed_at: completedAt,
      }).eq("id", reqRow.id);
      reqRow.status = "completed";
    }
  }

  return new Response(JSON.stringify({
    confirmation_code: reqRow.confirmation_code,
    status: reqRow.status,
    completed_at: completedAt,
  }), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
});
```

- [ ] **Step 3: Implement Desktop Deletion Sync and Scoped Local Anonymizer Matching Schema**

```rust
// src-tauri/src/conversation/facebook/deletion_sync.rs
use anyhow::Result;
use rusqlite::{params, Connection};
use std::fs;

pub struct FacebookDeletionWorker;

impl FacebookDeletionWorker {
    pub fn anonymize_user_data_local(conn: &mut Connection, channel_account_id: i64, external_user_id: &str) -> Result<usize> {
        let tx = conn.transaction()?;
        
        // 1. Anonymize leads scoped to this specific page and channel
        tx.execute(
            "UPDATE leads 
             SET name = 'Deleted User', email = NULL, phone = '', notes = '[Anonymized per GDPR/Meta privacy request]', updated_at = datetime('now')
             WHERE id IN (
                 SELECT lead_id FROM channel_identities 
                 WHERE channel_account_id = ?1 AND channel = 'messenger' AND identity_namespace = 'psid' AND external_id = ?2 AND lead_id IS NOT NULL
             )",
            params![channel_account_id, external_user_id],
        )?;

        // 2. Queue all attachment paths (both inbound and outbound) in the conversation into pending_spool_cleanups
        tx.execute(
            "INSERT INTO pending_spool_cleanups (local_path, created_at)
             SELECT local_path, datetime('now') FROM message_attachments 
             WHERE local_path IS NOT NULL AND message_id IN (
                 SELECT id FROM messages WHERE conversation_id IN (
                     SELECT id FROM channel_conversations WHERE channel_account_id = ?1 AND external_thread_id = ?2 AND kind = 'direct'
                 )
             )
             ON CONFLICT(local_path) DO NOTHING",
            params![channel_account_id, external_user_id],
        )?;

        // 3. Delete all attachments in the conversation
        tx.execute(
            "DELETE FROM message_attachments 
             WHERE message_id IN (
                 SELECT id FROM messages WHERE conversation_id IN (
                     SELECT id FROM channel_conversations WHERE channel_account_id = ?1 AND external_thread_id = ?2 AND kind = 'direct'
                 )
             )",
            params![channel_account_id, external_user_id],
        )?;

        // 4. Redact all messages in the conversation (both directions)
        tx.execute(
            "UPDATE messages 
             SET text_content = '[Deleted as per privacy request]', edited_at = datetime('now')
             WHERE conversation_id IN (
                 SELECT id FROM channel_conversations WHERE channel_account_id = ?1 AND external_thread_id = ?2 AND kind = 'direct'
             )",
            params![channel_account_id, external_user_id],
        )?;

        // 5. Delete receipts and reactions scoped strictly to this conversation and account
        tx.execute(
            "DELETE FROM message_receipts 
             WHERE message_id IN (
                 SELECT id FROM messages WHERE conversation_id IN (
                     SELECT id FROM channel_conversations WHERE channel_account_id = ?1 AND external_thread_id = ?2 AND kind = 'direct'
                 )
             )",
            params![channel_account_id, external_user_id],
        )?;

        tx.execute(
            "DELETE FROM message_reactions 
             WHERE message_id IN (
                 SELECT id FROM messages WHERE conversation_id IN (
                     SELECT id FROM channel_conversations WHERE channel_account_id = ?1 AND external_thread_id = ?2 AND kind = 'direct'
                 )
             )",
            params![channel_account_id, external_user_id],
        )?;

        tx.execute(
            "DELETE FROM pending_message_reactions WHERE channel_account_id = ?1 AND (actor_external_id = ?2 OR external_thread_id = ?2)",
            params![channel_account_id, external_user_id],
        )?;

        // 6. Anonymize comment logs
        tx.execute(
            "UPDATE facebook_comment_logs 
             SET sender_psid = 'ANONYMIZED', sender_name = 'Deleted User', comment_text = '[Deleted as per privacy request]', updated_at = datetime('now')
             WHERE page_id IN (SELECT page_id FROM facebook_page_tokens WHERE channel_account_id = ?1) AND sender_psid = ?2",
            params![channel_account_id, external_user_id],
        )?;

        // 7. Pseudonymize channel identities
        let rows = tx.execute(
            "UPDATE channel_identities 
             SET display_name = 'Deleted User', avatar_url = NULL, normalized_address = NULL, external_id = 'DELETED_' || id, updated_at = datetime('now')
             WHERE channel_account_id = ?1 AND channel = 'messenger' AND identity_namespace = 'psid' AND external_id = ?2",
            params![channel_account_id, external_user_id],
        )?;

        tx.commit()?;

        // Post-commit physical file deletion via pending_spool_cleanups
        let pending_cleanups: Vec<(i64, String)> = {
            let mut stmt = conn.prepare("SELECT id, local_path FROM pending_spool_cleanups")?;
            let r = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
            r.flatten().collect()
        };

        for (cleanup_id, path) in pending_cleanups {
            match fs::remove_file(&path) {
                Ok(_) => {
                    let _ = conn.execute("DELETE FROM pending_spool_cleanups WHERE id = ?1", [cleanup_id]);
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    let _ = conn.execute("DELETE FROM pending_spool_cleanups WHERE id = ?1", [cleanup_id]);
                }
                Err(_) => {
                    // Retain row in pending_spool_cleanups for future retry sweeper
                }
            }
        }

        Ok(rows)
    }

    pub async fn process_pending_device_tasks(
        conn: &mut Connection,
        supabase_url: &str,
        user_jwt: &str,
        device_id: &str,
    ) -> Result<usize> {
        let client = reqwest::Client::new();
        let get_url = format!("{}/rest/v1/meta_device_deletion_tasks?device_id=eq.{}&status=eq.pending", supabase_url, device_id);
        
        let resp = client.get(&get_url)
            .bearer_auth(user_jwt)
            .header("apikey", user_jwt)
            .send().await?;

        if !resp.status().is_success() {
            return Ok(0);
        }

        let tasks: Vec<serde_json::Value> = resp.json().await?;
        let mut completed_count = 0;

        for task in tasks {
            let task_id = task.get("id").and_then(|i| i.as_str()).unwrap_or("");
            let page_id = task.get("page_id").and_then(|p| p.as_str()).unwrap_or("");
            let user_id = task.get("user_id").and_then(|u| u.as_str()).unwrap_or("");

            if let Ok(channel_account_id) = conn.query_row(
                "SELECT channel_account_id FROM facebook_page_tokens WHERE page_id = ?1",
                params![page_id],
                |r| r.get::<_, i64>(0),
            ) {
                if let Ok(_) = Self::anonymize_user_data_local(conn, channel_account_id, user_id) {
                    let patch_url = format!("{}/rest/v1/meta_device_deletion_tasks?id=eq.{}", supabase_url, task_id);
                    let _ = client.patch(&patch_url)
                        .bearer_auth(user_jwt)
                        .header("apikey", user_jwt)
                        .json(&serde_json::json!({
                            "status": "completed",
                            "completed_at": chrono::Utc::now().to_rfc3339()
                        }))
                        .send().await;
                    completed_count += 1;
                }
            }
        }

        Ok(completed_count)
    }
}
```

- [ ] **Step 4: Commit Task 4**

```bash
git add supabase/ src-tauri/src/conversation/facebook/
git commit -m "feat(compliance): implement tenant-scoped data deletion lifecycle, transactional edge callback and post-commit local anonymizer via spool cleanups"
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
