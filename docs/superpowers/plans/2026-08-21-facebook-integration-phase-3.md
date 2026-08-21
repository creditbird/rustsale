# Facebook Integration Phase 3 Implementation Plan: Comment Automations, Lead Ads Ingestion, Page Health & Facebook Hub UI

- **Goal:** Deliver marketing & comment automations (Auto-Like, Public Reply, 1-reply Private Send API with 5-minute atomic claim lease & fail-closed reconciliation), Facebook Lead Ads ingestion with isolated cursor worker, Data Deletion Compliance Callback with Active-Device Filtering, Atomic Task Claim & Transactional Task-Scoped Anonymizer via `pending_spool_cleanups`, and Facebook Hub UI.
- **Architecture & Invariants:**
  - **Comment Automations:** Loop guard (`sender_id != page_id`), 7-day window. Permanent 4xx errors classified as `failed` with zero retries. Transient 5xx / timeouts mark `delivery_unknown`. Reconciler calls `GET /<comment_id>?fields=private_reply_status`.
  - **Data Deletion Compliance:** HMAC `signed_request` verification. PostgreSQL RPC `create_data_deletion_request_and_tasks` (service_role only) purges buffered events across all shapes and creates tasks for active registered tenant devices (`last_seen_at >= now() - INTERVAL '30 days'`).
  - **Desktop Deletion Worker:** Claims task atomically (`status='acknowledged'`) sending `anon_key` in `apikey` and `user_jwt` in `Authorization`. Records task-scoped spool paths in `pending_spool_cleanups(task_id, local_path)`. Only sends `completed` PATCH when all task-scoped files are confirmed removed from disk and PATCH returns HTTP 200.

---

### Task 1: Supabase Backend Compliance & Multi-Device Deletion

#### 1. SQL Migration (`supabase/migrations/20260821000003_meta_compliance.sql`)

```sql
CREATE TABLE IF NOT EXISTS tenant_registered_devices (
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    device_name TEXT,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(tenant_id, device_id)
);
ALTER TABLE tenant_registered_devices ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Tenants manage own devices" ON tenant_registered_devices
    FOR ALL USING (auth.uid() = tenant_id);

CREATE TABLE IF NOT EXISTS meta_data_deletion_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    confirmation_code TEXT NOT NULL UNIQUE,
    user_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
ALTER TABLE meta_data_deletion_requests ENABLE ROW LEVEL SECURITY;

CREATE TABLE IF NOT EXISTS meta_device_deletion_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL REFERENCES meta_data_deletion_requests(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'acknowledged', 'completed', 'failed')),
    acknowledged_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
ALTER TABLE meta_device_deletion_tasks ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Tenants read own device deletion tasks" ON meta_device_deletion_tasks
    FOR SELECT USING (auth.uid() = tenant_id);

CREATE POLICY "Tenants update own device deletion tasks" ON meta_device_deletion_tasks
    FOR UPDATE USING (auth.uid() = tenant_id);

CREATE OR REPLACE FUNCTION create_data_deletion_request_and_tasks(
    p_confirmation_code TEXT,
    p_user_id TEXT
)
RETURNS UUID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, auth, pg_temp
AS $$
DECLARE
    v_request_id UUID;
    v_tenant_id UUID;
    v_device_record RECORD;
    v_has_active_devices BOOLEAN := FALSE;
BEGIN
    INSERT INTO public.meta_data_deletion_requests (confirmation_code, user_id, status)
    VALUES (p_confirmation_code, p_user_id, 'processing')
    RETURNING id INTO v_request_id;

    -- Purge server-side buffered webhooks across all possible shapes
    DELETE FROM public.meta_webhook_buffer
    WHERE payload->'sender'->>'id' = p_user_id
       OR payload->'recipient'->>'id' = p_user_id
       OR payload->'from'->>'id' = p_user_id
       OR payload->'value'->'from'->>'id' = p_user_id
       OR payload->'value'->'sender'->>'id' = p_user_id;

    -- Fan out tasks to all active registered tenant devices (last seen within 30 days)
    FOR v_tenant_id IN 
        SELECT DISTINCT tenant_id FROM public.meta_user_page_interactions WHERE user_id = p_user_id
    LOOP
        FOR v_device_record IN 
            SELECT device_id FROM public.tenant_registered_devices 
            WHERE tenant_id = v_tenant_id AND last_seen_at >= now() - INTERVAL '30 days'
        LOOP
            v_has_active_devices := TRUE;
            INSERT INTO public.meta_device_deletion_tasks (request_id, tenant_id, device_id, user_id, status)
            VALUES (v_request_id, v_tenant_id, v_device_record.device_id, p_user_id, 'pending');
        END LOOP;

        DELETE FROM public.meta_user_page_interactions WHERE tenant_id = v_tenant_id AND user_id = p_user_id;
    END LOOP;

    -- If no active devices require local anonymization, mark request completed immediately
    IF NOT v_has_active_devices THEN
        UPDATE public.meta_data_deletion_requests SET status = 'completed', completed_at = now(), updated_at = now() WHERE id = v_request_id;
    END IF;

    RETURN v_request_id;
END;
$$;

REVOKE ALL ON FUNCTION create_data_deletion_request_and_tasks FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION create_data_deletion_request_and_tasks TO service_role;
```

#### 2. Compliance Edge Function (`supabase/functions/meta-data-deletion/index.ts`)

```typescript
import { serve } from "https://deno.land/std@0.177.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2.39.8";

const SUPABASE_URL = Deno.env.get("SUPABASE_URL")!;
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY")!;
const META_APP_SECRET = Deno.env.get("META_APP_SECRET")!;

const supabaseAdmin = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY);

async function parseAndVerifySignedRequest(signedRequest: string, appSecret: string): Promise<any | null> {
  const parts = signedRequest.split(".");
  if (parts.length !== 2) return null;
  const [encodedSig, payloadStr] = parts;

  try {
    // Base64URL decode signature bytes
    const sigBinaryStr = atob(encodedSig.replace(/-/g, "+").replace(/_/g, "/"));
    const sigBytes = new Uint8Array(sigBinaryStr.length);
    for (let i = 0; i < sigBinaryStr.length; i++) {
      sigBytes[i] = sigBinaryStr.charCodeAt(i);
    }

    // Compute HMAC-SHA256 of payloadStr using appSecret
    const encoder = new TextEncoder();
    const key = await crypto.subtle.importKey(
      "raw",
      encoder.encode(appSecret),
      { name: "HMAC", hash: "SHA-256" },
      false,
      ["verify"]
    );
    const isValid = await crypto.subtle.verify(
      "HMAC",
      key,
      sigBytes,
      encoder.encode(payloadStr)
    );

    if (!isValid) return null;

    const data = JSON.parse(atob(payloadStr.replace(/-/g, "+").replace(/_/g, "/")));
    if (data.algorithm !== "HMAC-SHA256") return null;
    return data;
  } catch {
    return null;
  }
}

serve(async (req: Request) => {
  const url = new URL(req.url);

  if (req.method === "POST" && url.pathname === "/meta-data-deletion") {
    const formData = await req.formData();
    const signedRequest = formData.get("signed_request") as string;
    if (!signedRequest) return new Response("Missing signed_request", { status: 400 });

    const data = await parseAndVerifySignedRequest(signedRequest, META_APP_SECRET);
    if (!data || !data.user_id) {
      return new Response("Invalid or untrusted signed_request signature", { status: 400 });
    }
    const userId = data.user_id;
    const confirmationCode = crypto.randomUUID();

    const { error: deletionRpcError } = await supabaseAdmin.rpc("create_data_deletion_request_and_tasks", {
      p_confirmation_code: confirmationCode,
      p_user_id: userId,
    });

    if (deletionRpcError) {
      console.error("Failed to create data deletion request:", deletionRpcError);
      return new Response("Internal Server Error: Failed to record deletion request", { status: 500 });
    }

    const statusUrl = `https://rustsale.trantrongtri.com/deletion-status?code=${confirmationCode}`;
    return new Response(JSON.stringify({ url: statusUrl, confirmation_code: confirmationCode }), {
      headers: { "Content-Type": "application/json" },
    });
  }

  if (req.method === "GET" && url.pathname === "/deletion-status") {
    const code = url.searchParams.get("code");
    if (!code) return new Response("Missing code", { status: 400 });

    const { data: reqData, error: reqError } = await supabaseAdmin
      .from("meta_data_deletion_requests")
      .select("id, status, created_at, completed_at")
      .eq("confirmation_code", code)
      .maybeSingle();

    if (reqError) {
      console.error("Failed to query deletion request:", reqError);
      return new Response("Internal Server Error: Status query failed", { status: 500 });
    }

    if (!reqData) {
      return new Response("Deletion request not found", { status: 404 });
    }

    const { data: pendingTasks, error: taskError } = await supabaseAdmin
      .from("meta_device_deletion_tasks")
      .select("id")
      .eq("request_id", reqData.id)
      .neq("status", "completed");

    if (taskError) {
      console.error("Failed to query device deletion task status:", taskError);
      return new Response("Internal Server Error: Device task check failed", { status: 500 });
    }

    let currentStatus = reqData.status;
    if (reqData.status === "processing" && (!pendingTasks || pendingTasks.length === 0)) {
      currentStatus = "completed";
      const { error: updateError } = await supabaseAdmin
        .from("meta_data_deletion_requests")
        .update({ status: "completed", completed_at: new Date().toISOString() })
        .eq("id", reqData.id);

      if (updateError) {
        console.error("Failed to update deletion request completion:", updateError);
        return new Response("Internal Server Error: Failed to record completion", { status: 500 });
      }
    }

    return new Response(JSON.stringify({
      status: currentStatus,
      confirmation_code: code,
      created_at: reqData.created_at,
      completed_at: currentStatus === "completed" ? (reqData.completed_at || new Date().toISOString()) : null,
    }), {
      headers: { "Content-Type": "application/json" },
    });
  }

  return new Response("Not Found", { status: 404 });
});
```

---

### Task 2: Desktop Worker Task Claim & Transactional Spool Anonymizer (`src-tauri/src/conversation/deletion_sync.rs`)

```rust
use anyhow::{anyhow, Result};
use reqwest::Client;
use rusqlite::Connection;
use serde_json::Value;

pub async fn process_pending_device_tasks(
    conn: &mut Connection,
    supabase_url: &str,
    anon_key: &str,
    user_jwt: &str,
    device_id: &str,
) -> Result<usize> {
    let client = Client::new();
    let lease_timeout_iso = (chrono::Utc::now() - chrono::Duration::minutes(15)).to_rfc3339();
    let tasks_url = format!("{}/rest/v1/meta_device_deletion_tasks?device_id=eq.{}&or=(status.eq.pending,and(status.eq.acknowledged,acknowledged_at.lt.{}))&select=id,user_id", supabase_url, device_id, lease_timeout_iso);

    let resp = client.get(&tasks_url)
        .header("apikey", anon_key)
        .header("Authorization", format!("Bearer {}", user_jwt))
        .send().await?;

    if !resp.status().is_success() {
        return Err(anyhow!("Failed to query device deletion tasks: {}", resp.status()));
    }

    let tasks: Vec<Value> = resp.json().await?;
    let mut completed_count = 0;

    for task in tasks {
        let task_id = task.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let user_id = task.get("user_id").and_then(|v| v.as_str()).unwrap_or("");
        if task_id.is_empty() || user_id.is_empty() {
            continue;
        }

        // 1. Atomic claim: pending (or expired acknowledged lease) -> acknowledged
        let claim_url = format!("{}/rest/v1/meta_device_deletion_tasks?id=eq.{}&or=(status.eq.pending,and(status.eq.acknowledged,acknowledged_at.lt.{}))", supabase_url, task_id, lease_timeout_iso);
        let claim_resp = client.patch(&claim_url)
            .header("apikey", anon_key)
            .header("Authorization", format!("Bearer {}", user_jwt))
            .header("Prefer", "return=representation")
            .json(&serde_json::json!({
                "status": "acknowledged",
                "acknowledged_at": chrono::Utc::now().to_rfc3339()
            }))
            .send().await?;

        if !claim_resp.status().is_success() {
            continue;
        }
        let claimed_rows: Vec<Value> = claim_resp.json().await.unwrap_or_default();
        if claimed_rows.is_empty() {
            continue;
        }

        // 2. Transactional local anonymization and spool registration
        {
            let tx = conn.transaction()?;

            // Find all attachments to delete belonging to messages sent by this messenger identity
            let mut att_stmt = tx.prepare(
                "SELECT ma.local_path 
                 FROM message_attachments ma 
                 JOIN messages m ON ma.message_id = m.id 
                 JOIN channel_identities ci ON m.sender_identity_id = ci.id 
                 WHERE ci.channel = 'messenger' AND ci.external_id = ?1 AND ma.local_path IS NOT NULL AND TRIM(ma.local_path) != ''"
            )?;
            let paths: Vec<String> = att_stmt
                .query_map([user_id], |r| r.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            for path in &paths {
                tx.execute(
                    "INSERT OR IGNORE INTO pending_spool_cleanups (local_path, created_at) VALUES (?1, datetime('now'))",
                    rusqlite::params![path],
                )?;
            }

            // Wipe all attachment identifiers, remote URLs, and metadata
            tx.execute(
                "UPDATE message_attachments 
                 SET provider_attachment_id = NULL,
                     remote_url = NULL,
                     thumbnail_url = NULL,
                     file_name = '[Deleted]',
                     local_path = NULL,
                     metadata_json = '{}'
                 WHERE message_id IN (
                     SELECT m.id FROM messages m
                     JOIN channel_identities ci ON m.sender_identity_id = ci.id
                     WHERE ci.channel = 'messenger' AND ci.external_id = ?1
                 )",
                [user_id],
            )?;

            // Redact messages
            tx.execute(
                "UPDATE messages 
                 SET text_content = '[Deleted for Compliance]',
                     content_json = '{\"type\":\"text\",\"text\":\"[Deleted for Compliance]\"}',
                     raw_payload_json = NULL
                 WHERE sender_identity_id IN (
                     SELECT id FROM channel_identities WHERE channel = 'messenger' AND external_id = ?1
                 )",
                [user_id],
            )?;

            // Anonymize identity including external_id tombstone (without nonexistent metadata_json column)
            tx.execute(
                "UPDATE channel_identities 
                 SET display_name = 'Deleted User', 
                     avatar_url = NULL, 
                     normalized_address = NULL,
                     external_id = 'deleted_' || id || '_' || hex(randomblob(4)),
                     profile_synced_at = datetime('now'),
                     profile_sync_error = NULL,
                     updated_at = datetime('now') 
                 WHERE channel = 'messenger' AND external_id = ?1",
                [user_id],
            )?;

            tx.commit()?;
        }

        // 3. Durable spool sweep from disk using pending_spool_cleanups table
        let pending_cleanups: Vec<String> = {
            let mut stmt = conn.prepare("SELECT local_path FROM pending_spool_cleanups")?;
            stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<Vec<_>>>()?
        };

        let mut all_cleaned = true;
        for path in &pending_cleanups {
            if std::path::Path::new(path).exists() {
                if std::fs::remove_file(path).is_ok() {
                    conn.execute("DELETE FROM pending_spool_cleanups WHERE local_path = ?1", rusqlite::params![path])?;
                } else {
                    all_cleaned = false;
                }
            } else {
                conn.execute("DELETE FROM pending_spool_cleanups WHERE local_path = ?1", rusqlite::params![path])?;
            }
        }

        let remaining_spools: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pending_spool_cleanups",
            [],
            |r| r.get(0),
        )?;

        // 4. Update task status on Supabase: completed only when clean and 0 pending spools remain
        if all_cleaned && remaining_spools == 0 {
            let complete_url = format!("{}/rest/v1/meta_device_deletion_tasks?id=eq.{}&status=eq.acknowledged", supabase_url, task_id);
            let patch_resp = client.patch(&complete_url)
                .header("apikey", anon_key)
                .header("Authorization", format!("Bearer {}", user_jwt))
                .header("Prefer", "return=representation")
                .json(&serde_json::json!({
                    "status": "completed",
                    "completed_at": chrono::Utc::now().to_rfc3339()
                }))
                .send().await?;

            if patch_resp.status().is_success() {
                completed_count += 1;
            }
        } else {
            // Reset task to pending so next cycle retries disk cleanup without losing state
            let reset_url = format!("{}/rest/v1/meta_device_deletion_tasks?id=eq.{}&status=eq.acknowledged", supabase_url, task_id);
            let _ = client.patch(&reset_url)
                .header("apikey", anon_key)
                .header("Authorization", format!("Bearer {}", user_jwt))
                .json(&serde_json::json!({
                    "status": "pending",
                    "acknowledged_at": serde_json::Value::Null
                }))
                .send().await;
        }
    }

    Ok(completed_count)
}
```
