# Facebook Integration Phase 3 Implementation Plan: Comment Automations, Lead Ads Ingestion, Page Health & Facebook Hub UI

- **Goal:** Deliver marketing & comment automations (Auto-Like, Public Reply, 1-reply Private Send API with 5-minute atomic claim lease & fail-closed reconciliation), Facebook Lead Ads ingestion with isolated cursor worker, Data Deletion Compliance Callback with Active-Device Filtering, Atomic Task Claim & Transactional Task-Scoped Anonymizer via `meta_deletion_pending_cleanups`, and Facebook Hub UI.
- **Architecture & Invariants:**
  - **Comment Automations:** Loop guard (`sender_id != page_id`), 7-day window. Permanent 4xx errors classified as `failed` with zero retries. Transient 5xx / timeouts mark `delivery_unknown`. Reconciler calls `GET /<comment_id>?fields=private_reply_status`.
  - **Data Deletion Compliance:** HMAC `signed_request` verification. PostgreSQL RPC `create_data_deletion_request_and_tasks` (service_role only) purges buffered events across all shapes and creates tasks for active registered tenant devices (`last_seen_at >= now() - INTERVAL '30 days'`).
  - **Desktop Deletion Worker:** Claims task atomically via `claim_device_deletion_tasks` receiving a single-use `lease_token`. Records task-scoped spool paths in `meta_deletion_pending_cleanups(task_id, local_path)`. Only calls `complete_device_deletion_task` when all task-scoped files are confirmed removed from disk and active references verified with fail-closed safety.

---

### Task 1: Supabase Backend Compliance & Multi-Device Deletion

#### 1. SQL Migration (`supabase/migrations/20260821000003_meta_compliance.sql`)

```sql
CREATE TABLE IF NOT EXISTS tenant_registered_devices (
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    device_secret_hash TEXT NOT NULL,
    device_name TEXT,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(tenant_id, device_id)
);
ALTER TABLE tenant_registered_devices ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE tenant_registered_devices FROM PUBLIC, anon, authenticated;
GRANT SELECT (tenant_id, device_id, device_name, last_seen_at) ON TABLE tenant_registered_devices TO authenticated;
GRANT ALL ON TABLE tenant_registered_devices TO service_role;

CREATE POLICY "Tenants manage own devices" ON tenant_registered_devices
    FOR SELECT TO authenticated USING (auth.uid() = tenant_id);

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

CREATE TABLE IF NOT EXISTS meta_tenant_deletion_tombstones (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    page_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, page_id, user_id)
);
ALTER TABLE meta_tenant_deletion_tombstones ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE meta_tenant_deletion_tombstones FROM PUBLIC, anon, authenticated;
GRANT SELECT ON TABLE meta_tenant_deletion_tombstones TO authenticated;
GRANT ALL ON TABLE meta_tenant_deletion_tombstones TO service_role;

CREATE POLICY "Tenants read own deletion tombstones" ON meta_tenant_deletion_tombstones
    FOR SELECT TO authenticated USING (auth.uid() = tenant_id);

CREATE TABLE IF NOT EXISTS meta_device_deletion_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL REFERENCES meta_data_deletion_requests(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    page_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'acknowledged', 'completed', 'failed')),
    lease_token_hash TEXT,
    acknowledged_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
ALTER TABLE meta_device_deletion_tasks ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE meta_device_deletion_tasks FROM PUBLIC, anon, authenticated;
GRANT SELECT (id, request_id, tenant_id, device_id, page_id, user_id, status, acknowledged_at, completed_at, created_at, updated_at) ON TABLE meta_device_deletion_tasks TO authenticated;
GRANT ALL ON TABLE meta_device_deletion_tasks TO service_role;

CREATE POLICY "Tenants read own device deletion tasks" ON meta_device_deletion_tasks
    FOR SELECT TO authenticated USING (auth.uid() = tenant_id);

CREATE OR REPLACE FUNCTION claim_device_deletion_tasks(
    p_device_id TEXT,
    p_device_secret TEXT,
    p_lease_seconds INT DEFAULT 300
)
RETURNS TABLE (
    id UUID,
    request_id UUID,
    page_id TEXT,
    user_id TEXT,
    lease_token TEXT,
    created_at TIMESTAMPTZ
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, auth, extensions, pg_temp
AS $$
DECLARE
    v_raw_token TEXT;
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM public.tenant_registered_devices
        WHERE tenant_id = auth.uid()
          AND device_id = p_device_id
          AND device_secret_hash = encode(digest(p_device_secret, 'sha256'), 'hex')
    ) THEN
        RAISE EXCEPTION 'Unauthorized: Invalid device credentials';
    END IF;

    v_raw_token := encode(gen_random_bytes(32), 'hex');

    RETURN QUERY
    UPDATE public.meta_device_deletion_tasks
    SET status = 'acknowledged',
        lease_token_hash = encode(digest(v_raw_token, 'sha256'), 'hex'),
        acknowledged_at = now(),
        updated_at = now()
    WHERE meta_device_deletion_tasks.id IN (
        SELECT t.id FROM public.meta_device_deletion_tasks t
        WHERE t.tenant_id = auth.uid()
          AND t.device_id = p_device_id
          AND (
              t.status = 'pending'
              OR (t.status = 'acknowledged' AND t.acknowledged_at < now() - (p_lease_seconds || ' seconds')::INTERVAL)
          )
        ORDER BY t.created_at ASC
        LIMIT 10
        FOR UPDATE SKIP LOCKED
    )
    RETURNING meta_device_deletion_tasks.id, meta_device_deletion_tasks.request_id, meta_device_deletion_tasks.page_id, meta_device_deletion_tasks.user_id, v_raw_token AS lease_token, meta_device_deletion_tasks.created_at;
END;
$$;

CREATE OR REPLACE FUNCTION complete_device_deletion_task(
    p_task_id UUID,
    p_device_id TEXT,
    p_device_secret TEXT,
    p_lease_token TEXT
)
RETURNS BOOLEAN
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, auth, extensions, pg_temp
AS $$
DECLARE
    v_updated INT;
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM public.tenant_registered_devices
        WHERE tenant_id = auth.uid()
          AND device_id = p_device_id
          AND device_secret_hash = encode(digest(p_device_secret, 'sha256'), 'hex')
    ) THEN
        RAISE EXCEPTION 'Unauthorized: Invalid device credentials';
    END IF;

    UPDATE public.meta_device_deletion_tasks
    SET status = 'completed',
        completed_at = now(),
        lease_token_hash = NULL,
        updated_at = now()
    WHERE id = p_task_id
      AND tenant_id = auth.uid()
      AND device_id = p_device_id
      AND lease_token_hash = encode(digest(p_lease_token, 'sha256'), 'hex')
      AND status = 'acknowledged';

    GET DIAGNOSTICS v_updated = ROW_COUNT;
    RETURN v_updated > 0;
END;
$$;

CREATE OR REPLACE FUNCTION release_device_deletion_task(
    p_task_id UUID,
    p_device_id TEXT,
    p_device_secret TEXT,
    p_lease_token TEXT
)
RETURNS BOOLEAN
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, auth, extensions, pg_temp
AS $$
DECLARE
    v_updated INT;
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM public.tenant_registered_devices
        WHERE tenant_id = auth.uid()
          AND device_id = p_device_id
          AND device_secret_hash = encode(digest(p_device_secret, 'sha256'), 'hex')
    ) THEN
        RAISE EXCEPTION 'Unauthorized: Invalid device credentials';
    END IF;

    UPDATE public.meta_device_deletion_tasks
    SET status = 'pending',
        lease_token_hash = NULL,
        acknowledged_at = NULL,
        updated_at = now()
    WHERE id = p_task_id
      AND tenant_id = auth.uid()
      AND device_id = p_device_id
      AND lease_token_hash = encode(digest(p_lease_token, 'sha256'), 'hex')
      AND status = 'acknowledged';

    GET DIAGNOSTICS v_updated = ROW_COUNT;
    RETURN v_updated > 0;
END;
$$;

CREATE OR REPLACE FUNCTION register_or_heartbeat_device(
    p_device_id TEXT,
    p_device_secret TEXT,
    p_device_name TEXT DEFAULT NULL
)
RETURNS BOOLEAN
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, auth, extensions, pg_temp
AS $$
DECLARE
    v_secret_hash TEXT;
    v_existing_hash TEXT;
BEGIN
    IF p_device_secret IS NULL OR length(p_device_secret) < 32 THEN
        RAISE EXCEPTION 'Invalid device secret: minimum 32 characters required';
    END IF;

    v_secret_hash := encode(digest(p_device_secret, 'sha256'), 'hex');

    SELECT device_secret_hash INTO v_existing_hash
    FROM public.tenant_registered_devices
    WHERE tenant_id = auth.uid() AND device_id = p_device_id;

    IF v_existing_hash IS NOT NULL THEN
        IF v_existing_hash != v_secret_hash THEN
            RAISE EXCEPTION 'Unauthorized: Device credential mismatch';
        END IF;
        UPDATE public.tenant_registered_devices
        SET last_seen_at = now(),
            device_name = COALESCE(p_device_name, device_name)
        WHERE tenant_id = auth.uid() AND device_id = p_device_id;
    ELSE
        INSERT INTO public.tenant_registered_devices (tenant_id, device_id, device_secret_hash, device_name, last_seen_at)
        VALUES (auth.uid(), p_device_id, v_secret_hash, p_device_name, now());
    END IF;

    -- Durable scrub-on-reconnect: Backfill pending deletion tasks for any tenant deletion tombstones not yet completed by this device
    INSERT INTO public.meta_device_deletion_tasks (request_id, tenant_id, device_id, page_id, user_id, status)
    SELECT COALESCE((SELECT r.id FROM public.meta_data_deletion_requests r WHERE r.status = 'processing' LIMIT 1), gen_random_uuid()),
           t.tenant_id, p_device_id, t.page_id, t.user_id, 'pending'
    FROM public.meta_tenant_deletion_tombstones t
    WHERE t.tenant_id = auth.uid()
      AND NOT EXISTS (
          SELECT 1 FROM public.meta_device_deletion_tasks dt
          WHERE dt.tenant_id = auth.uid() AND dt.device_id = p_device_id AND dt.page_id = t.page_id AND dt.user_id = t.user_id AND dt.status = 'completed'
      );

    RETURN TRUE;
END;
$$;

REVOKE ALL ON FUNCTION register_or_heartbeat_device, claim_device_deletion_tasks, complete_device_deletion_task, release_device_deletion_task FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION register_or_heartbeat_device, claim_device_deletion_tasks, complete_device_deletion_task, release_device_deletion_task TO authenticated, service_role;

CREATE OR REPLACE FUNCTION create_data_deletion_request_and_tasks(
    p_confirmation_code TEXT,
    p_user_id TEXT,
    p_target_mappings JSONB DEFAULT '[]'::JSONB
)
RETURNS UUID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, auth, pg_temp
AS $$
DECLARE
    v_request_id UUID;
    v_tenant_record RECORD;
    v_device_record RECORD;
    v_has_active_devices BOOLEAN := FALSE;
BEGIN
    INSERT INTO public.meta_data_deletion_requests (confirmation_code, user_id, status)
    VALUES (p_confirmation_code, p_user_id, 'processing')
    RETURNING id INTO v_request_id;

    -- Collect all distinct target (tenant_id, page_id, page_scoped_user_id) tuples matching either root app_user_id or exact (page_id, psid) mapping
    FOR v_tenant_record IN
        SELECT DISTINCT mupi.tenant_id, mupi.page_id, mupi.page_scoped_user_id
        FROM public.meta_user_page_interactions mupi
        WHERE mupi.app_user_id = p_user_id
           OR EXISTS (
               SELECT 1 FROM jsonb_array_elements(p_target_mappings) m
               WHERE (m->>'page_id' IS NOT NULL AND m->>'page_id' <> '')
                 AND mupi.page_id = (m->>'page_id')
                 AND mupi.page_scoped_user_id = (m->>'psid')
           )
    LOOP
        -- Persist durable tenant-scoped deletion tombstone for scrub-on-reconnect
        INSERT INTO public.meta_tenant_deletion_tombstones (tenant_id, page_id, user_id)
        VALUES (v_tenant_record.tenant_id, v_tenant_record.page_id, v_tenant_record.page_scoped_user_id)
        ON CONFLICT (tenant_id, page_id, user_id) DO NOTHING;

        -- Purge server-side buffered webhooks strictly scoped to this tenant_id and page_id
        DELETE FROM public.meta_webhook_buffer
        WHERE tenant_id = v_tenant_record.tenant_id
          AND (page_id = v_tenant_record.page_id OR page_id IS NULL)
          AND (
              payload->'sender'->>'id' = v_tenant_record.page_scoped_user_id
              OR payload->'recipient'->>'id' = v_tenant_record.page_scoped_user_id
              OR payload->'from'->>'id' = v_tenant_record.page_scoped_user_id
              OR payload->'value'->'from'->>'id' = v_tenant_record.page_scoped_user_id
              OR payload->'value'->'sender'->>'id' = v_tenant_record.page_scoped_user_id
              OR payload->'value'->>'sender_id' = v_tenant_record.page_scoped_user_id
              OR payload->'value'->>'user_id' = v_tenant_record.page_scoped_user_id
          );

        -- Fan out tasks to all registered tenant devices regardless of offline duration
        FOR v_device_record IN
            SELECT device_id FROM public.tenant_registered_devices
            WHERE tenant_id = v_tenant_record.tenant_id
        LOOP
            v_has_active_devices := TRUE;
            -- Fan out deletion task strictly carrying page_id and page-scoped user_id to be scrubbed on desktop SQLite
            INSERT INTO public.meta_device_deletion_tasks (request_id, tenant_id, device_id, page_id, user_id, status)
            VALUES (v_request_id, v_tenant_record.tenant_id, v_device_record.device_id, v_tenant_record.page_id, v_tenant_record.page_scoped_user_id, 'pending');
        END LOOP;

        -- Clean up interaction records strictly scoped to this tenant, page, and user
        DELETE FROM public.meta_user_page_interactions
        WHERE tenant_id = v_tenant_record.tenant_id
          AND page_id = v_tenant_record.page_id
          AND (app_user_id = p_user_id OR page_scoped_user_id = v_tenant_record.page_scoped_user_id);
    END LOOP;

    -- Clean up any residual global interaction records matching app_user_id
    DELETE FROM public.meta_user_page_interactions WHERE app_user_id = p_user_id;

    -- If no registered devices exist for target tenants, mark request completed immediately
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
const META_APP_ID = Deno.env.get("META_APP_ID")!;
const META_APP_SECRET = Deno.env.get("META_APP_SECRET")!;

const supabaseAdmin = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY);

interface PageScopedMapping {
  page_id: string;
  psid: string;
}

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

async function resolvePsidsForAsid(asid: string): Promise<PageScopedMapping[]> {
  if (!META_APP_ID || !META_APP_SECRET) {
    throw new Error("Missing Meta App credentials (META_APP_ID or META_APP_SECRET)");
  }
  const mappings: PageScopedMapping[] = [];
  const appToken = `${META_APP_ID}|${META_APP_SECRET}`;
  let nextUrl: string | null = `https://graph.facebook.com/v26.0/${encodeURIComponent(asid)}/ids_for_pages?access_token=${appToken}&limit=100`;

  while (nextUrl) {
    const res = await fetch(nextUrl);
    if (!res.ok) {
      const errBody = await res.text().catch(() => "");
      throw new Error(`Meta Graph API ids_for_pages returned HTTP ${res.status}: ${errBody}`);
    }
    const body = await res.json();
    if (Array.isArray(body?.data)) {
      for (const item of body.data) {
        const psid = item?.id ? String(item.id).trim() : "";
        const pageId = item?.page?.id ? String(item.page.id).trim() : "";
        // Strict fail-closed validation: Reject mappings missing page_id or psid
        if (psid && pageId) {
          mappings.push({ page_id: pageId, psid });
        }
      }
    }
    nextUrl = body?.paging?.next || null;
  }

  return mappings;
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

    // Authoritatively resolve all Page-Scoped IDs (PSIDs) and associated page_ids for this App-Scoped ID (ASID)
    let resolvedMappings: PageScopedMapping[];
    try {
      resolvedMappings = await resolvePsidsForAsid(userId);
    } catch (err) {
      console.error("Fail-closed: Failed to resolve PSIDs from Meta Graph API for ASID:", userId, err);
      return new Response("Bad Gateway: Unable to resolve user identities from Meta Graph API. Retry later.", { status: 502 });
    }

    // Fail-closed check on empty resolution: verify whether any retained interaction exists for app_user_id
    if (resolvedMappings.length === 0) {
      const { count, error: countErr } = await supabaseAdmin
        .from("meta_user_page_interactions")
        .select("*", { count: "exact", head: true })
        .eq("app_user_id", userId);
      if (countErr || !count || count === 0) {
        return new Response("Bad Request: No associated Page identities or retained records found for ASID", { status: 400 });
      }
    }

    const { error: deletionRpcError } = await supabaseAdmin.rpc("create_data_deletion_request_and_tasks", {
      p_confirmation_code: confirmationCode,
      p_user_id: userId,
      p_target_mappings: resolvedMappings,
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
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

pub async fn process_pending_device_tasks(
    conn: &mut Connection,
    supabase_url: &str,
    anon_key: &str,
    user_jwt: &str,
    device_id: &str,
    device_secret: &str,
    spool_root: &std::path::Path,
) -> Result<usize> {
    let client = Client::new();

    // 0. Ensure device is enrolled and active via register_or_heartbeat_device RPC
    let heartbeat_url = format!("{}/rest/v1/rpc/register_or_heartbeat_device", supabase_url);
    let heartbeat_resp = client.post(&heartbeat_url)
        .header("apikey", anon_key)
        .header("Authorization", format!("Bearer {}", user_jwt))
        .json(&serde_json::json!({
            "p_device_id": device_id,
            "p_device_secret": device_secret,
            "p_device_name": "RustSale Desktop"
        }))
        .send().await?;

    if !heartbeat_resp.status().is_success() {
        return Err(anyhow!("Failed to register/heartbeat device before claiming tasks: {}", heartbeat_resp.text().await?));
    }

    // 1. Claim pending deletion tasks via secure RPC (bound to authenticated device_secret and returning unforgeable single-use lease tokens)
    let claim_url = format!("{}/rest/v1/rpc/claim_device_deletion_tasks", supabase_url);
    let claim_resp = client.post(&claim_url)
        .header("apikey", anon_key)
        .header("Authorization", format!("Bearer {}", user_jwt))
        .json(&serde_json::json!({
            "p_device_id": device_id,
            "p_device_secret": device_secret,
            "p_lease_seconds": 300
        }))
        .send().await?;

    if !claim_resp.status().is_success() {
        return Err(anyhow!("Failed to claim device deletion tasks: {}", claim_resp.status()));
    }

    let tasks: Vec<Value> = claim_resp.json().await?;
    let mut completed_count = 0;

    for task in tasks {
        let task_id = task.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let page_id = task.get("page_id").and_then(|v| v.as_str()).unwrap_or("");
        let user_id = task.get("user_id").and_then(|v| v.as_str()).unwrap_or("");
        let lease_token = task.get("lease_token").and_then(|v| v.as_str()).unwrap_or("");
        if task_id.is_empty() || page_id.is_empty() || user_id.is_empty() || lease_token.is_empty() {
            continue;
        }

        // 2. Transactional local anonymization and task-scoped spool registration strictly scoped to page_id / channel_account_id (fail-closed)
        {
            let tx = conn.transaction()?;

            // Resolve exactly one authoritative Messenger account joined to facebook_page_tokens for this Page ID
            let matched_accounts: Vec<i64> = {
                let mut stmt = tx.prepare(
                    "SELECT ca.id
                     FROM channel_accounts ca
                     JOIN facebook_page_tokens fpt ON ca.id = fpt.channel_account_id
                     WHERE ca.channel = 'messenger' AND fpt.page_id = ?1 AND ca.provider_account_key = ?1"
                )?;
                stmt.query_map([page_id], |r| r.get(0))?.collect::<rusqlite::Result<Vec<_>>>()?
            };

            let account_id = match matched_accounts.as_slice() {
                [single_id] => *single_id,
                [] => return Err(anyhow!("Fail-closed: No authoritative Messenger channel account found for page_id {}", page_id)),
                _ => return Err(anyhow!("Fail-closed: Ambiguous mapping: multiple Messenger channel accounts found for page_id {}", page_id)),
            };

            // Find all attachments to delete belonging to Messenger messages sent by target user in this Page account
            let mut att_stmt = tx.prepare(
                "SELECT ma.local_path
                 FROM message_attachments ma
                 JOIN messages m ON ma.message_id = m.id
                 JOIN channel_conversations cc ON m.conversation_id = cc.id
                 WHERE cc.channel_account_id = ?1
                   AND m.sender_identity_id IN (
                       SELECT id FROM channel_identities WHERE channel_account_id = ?1 AND channel = 'messenger' AND external_id = ?2
                   )
                   AND ma.local_path IS NOT NULL AND TRIM(ma.local_path) != ''"
            )?;
            let paths: Vec<String> = att_stmt
                .query_map(rusqlite::params![account_id, user_id], |r| r.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            for path in &paths {
                tx.execute(
                    "INSERT OR IGNORE INTO meta_deletion_pending_cleanups (task_id, local_path, created_at) VALUES (?1, ?2, datetime('now'))",
                    rusqlite::params![task_id, path],
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
                     JOIN channel_conversations cc ON m.conversation_id = cc.id
                     WHERE cc.channel_account_id = ?1
                       AND m.sender_identity_id IN (
                           SELECT id FROM channel_identities WHERE channel_account_id = ?1 AND channel = 'messenger' AND external_id = ?2
                       )
                 )",
                rusqlite::params![account_id, user_id],
            )?;

            // Scrub residual user identifiers across Messenger reactions, receipts, pending reactions, and mentions for this account
            tx.execute(
                "DELETE FROM message_reactions
                 WHERE actor_external_id = ?2
                   AND message_id IN (
                       SELECT m.id FROM messages m
                       JOIN channel_conversations cc ON m.conversation_id = cc.id
                       WHERE cc.channel_account_id = ?1
                   )",
                rusqlite::params![account_id, user_id],
            )?;
            tx.execute(
                "DELETE FROM message_receipts
                 WHERE recipient_external_id = ?2
                   AND message_id IN (
                       SELECT m.id FROM messages m
                       JOIN channel_conversations cc ON m.conversation_id = cc.id
                       WHERE cc.channel_account_id = ?1
                   )",
                rusqlite::params![account_id, user_id],
            )?;
            tx.execute(
                "DELETE FROM pending_message_reactions
                 WHERE channel_account_id = ?1 AND actor_external_id = ?2",
                rusqlite::params![account_id, user_id],
            )?;
            tx.execute(
                "DELETE FROM message_mentions
                 WHERE (external_id = ?2 OR identity_id IN (SELECT id FROM channel_identities WHERE channel_account_id = ?1 AND channel = 'messenger' AND external_id = ?2))
                   AND message_id IN (
                       SELECT m.id FROM messages m
                       JOIN channel_conversations cc ON m.conversation_id = cc.id
                       WHERE cc.channel_account_id = ?1
                   )",
                rusqlite::params![account_id, user_id],
            )?;

            // Scrub channel sync dead letters for this Messenger user and account across all structured payload shapes
            tx.execute(
                "DELETE FROM channel_sync_dead_letters
                 WHERE channel_account_id = ?1 AND (
                     event_key = ?2
                     OR json_extract(payload_json, '$.sender.id') = ?2
                     OR json_extract(payload_json, '$.recipient.id') = ?2
                     OR json_extract(payload_json, '$.from.id') = ?2
                     OR json_extract(payload_json, '$.sender_id') = ?2
                     OR json_extract(payload_json, '$.user_id') = ?2
                     OR json_extract(payload_json, '$.value.from.id') = ?2
                     OR json_extract(payload_json, '$.value.sender_id') = ?2
                     OR json_extract(payload_json, '$.value.user_id') = ?2
                     OR json_extract(payload_json, '$.value.sender.id') = ?2
                     OR json_extract(payload_json, '$.entry[0].messaging[0].sender.id') = ?2
                     OR json_extract(payload_json, '$.entry[0].messaging[0].recipient.id') = ?2
                     OR json_extract(payload_json, '$.entry[0].changes[0].value.from.id') = ?2
                     OR json_extract(payload_json, '$.entry[0].changes[0].value.sender_id') = ?2
                     OR json_extract(payload_json, '$.entry[0].changes[0].value.user_id') = ?2
                 )",
                rusqlite::params![account_id, user_id],
            )?;

            // Redact messages
            tx.execute(
                "UPDATE messages
                 SET text_content = '[Deleted for Compliance]',
                     content_json = '{\"type\":\"text\",\"text\":\"[Deleted for Compliance]\"}',
                     raw_payload_json = NULL
                 WHERE conversation_id IN (
                     SELECT cc.id FROM channel_conversations cc
                     WHERE cc.channel_account_id = ?1
                 ) AND sender_identity_id IN (
                     SELECT id FROM channel_identities WHERE channel_account_id = ?1 AND channel = 'messenger' AND external_id = ?2
                 )",
                rusqlite::params![account_id, user_id],
            )?;

            // Anonymize identity including external_id tombstone scoped to this channel account
            tx.execute(
                "UPDATE channel_identities
                 SET display_name = 'Deleted User',
                     avatar_url = NULL,
                     normalized_address = NULL,
                     external_id = 'deleted_' || id || '_' || hex(randomblob(4)),
                     profile_synced_at = datetime('now'),
                     profile_sync_error = NULL,
                     updated_at = datetime('now')
                 WHERE channel_account_id = ?1 AND channel = 'messenger' AND external_id = ?2",
                rusqlite::params![account_id, user_id],
            )?;

            // Scrub Facebook comment logs matching page_id and sender_psid
            tx.execute(
                "DELETE FROM facebook_comment_logs WHERE page_id = ?1 AND sender_psid = ?2",
                rusqlite::params![page_id, user_id],
            )?;

            tx.commit()?;
        }

        // 3. Task-scoped spool sweep from disk with canonical path, try_exists error preservation, and fail-closed active reference validation
        let pending_cleanups: Vec<String> = {
            let mut stmt = conn.prepare("SELECT local_path FROM meta_deletion_pending_cleanups WHERE task_id = ?1")?;
            stmt.query_map([task_id], |r| r.get(0))?.collect::<rusqlite::Result<Vec<_>>>()?
        };

        let mut all_cleaned = true;
        for path in &pending_cleanups {
            let p = std::path::Path::new(path);
            let is_canonical_spool = p.canonicalize().ok()
                .map(|canon| canon.starts_with(spool_root))
                .unwrap_or(false);

            let active_refs: Result<i64, _> = conn.query_row(
                "SELECT COUNT(*) FROM message_attachments WHERE local_path = ?1",
                [path],
                |r| r.get(0),
            );

            match active_refs {
                Ok(0) => {
                    match p.try_exists() {
                        Ok(true) => {
                            if is_canonical_spool && std::fs::remove_file(p).is_ok() {
                                conn.execute("DELETE FROM meta_deletion_pending_cleanups WHERE task_id = ?1 AND local_path = ?2", rusqlite::params![task_id, path])?;
                            } else {
                                all_cleaned = false;
                            }
                        }
                        Ok(false) => {
                            // File strictly absent from disk, safely purge queue entry
                            conn.execute("DELETE FROM meta_deletion_pending_cleanups WHERE task_id = ?1 AND local_path = ?2", rusqlite::params![task_id, path])?;
                        }
                        Err(err) => {
                            // Permission denied or I/O error accessing metadata: FAIL CLOSED
                            tracing::error!("Filesystem try_exists error for path {}: {}", path, err);
                            all_cleaned = false;
                        }
                    }
                }
                Ok(_) => {
                    // Retain shared file on disk for other active records, clean up task queue entry safely
                    conn.execute("DELETE FROM meta_deletion_pending_cleanups WHERE task_id = ?1 AND local_path = ?2", rusqlite::params![task_id, path])?;
                }
                Err(err) => {
                    tracing::error!("Failed to check active references for path {}: {}", path, err);
                    all_cleaned = false;
                }
            }
        }

        let remaining_spools: i64 = conn.query_row(
            "SELECT COUNT(*) FROM meta_deletion_pending_cleanups WHERE task_id = ?1",
            [task_id],
            |r| r.get(0),
        )?;

        // 4. Update task status on Supabase via single-use lease token & device-secret RPCs
        if all_cleaned && remaining_spools == 0 {
            let complete_url = format!("{}/rest/v1/rpc/complete_device_deletion_task", supabase_url);
            let patch_resp = client.post(&complete_url)
                .header("apikey", anon_key)
                .header("Authorization", format!("Bearer {}", user_jwt))
                .json(&serde_json::json!({
                    "p_task_id": task_id,
                    "p_device_id": device_id,
                    "p_device_secret": device_secret,
                    "p_lease_token": lease_token
                }))
                .send().await?;

            if patch_resp.status().is_success() {
                let is_completed: bool = patch_resp.json().await.unwrap_or(false);
                if is_completed {
                    completed_count += 1;
                }
            }
        } else {
            let release_url = format!("{}/rest/v1/rpc/release_device_deletion_task", supabase_url);
            let _ = client.post(&release_url)
                .header("apikey", anon_key)
                .header("Authorization", format!("Bearer {}", user_jwt))
                .json(&serde_json::json!({
                    "p_task_id": task_id,
                    "p_device_id": device_id,
                    "p_device_secret": device_secret,
                    "p_lease_token": lease_token
                }))
                .send().await;
        }
    }

    Ok(completed_count)
}
```
