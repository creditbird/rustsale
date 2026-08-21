# Facebook Integration (Phase 1: Foundation, OAuth & Messenger 2-Way Sync) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish the production foundation for Facebook Integration: zero-secret PKCE OAuth token exchange, encrypted local credential store, durable Supabase webhook relay buffer, isolated multi-stream sync engine for Messenger 2-way conversation matching repository schema (`channel_conversations`, `conversation_participants`, `messages`, `message_attachments`, `message_reactions`, `message_receipts`, `pending_message_reactions`), and Svelte 5 chat UI with 24-hour Message Tag policy enforcement.

**Tech Stack:** Tauri 2.0 (Rust), Svelte 5 (Runes), SQLite Local (`rusqlite`), Supabase Edge Functions (Deno / TypeScript), Meta Graph API v26.0+, Tailwind CSS.

---

## 1. Mandatory Implementation Harness Ledger (Tier 3)

| Req ID | Requirement | Status | Owning Step | Boundary Trace & Invariants | Adversarial Counterexample / Proof | Verification Method |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **REQ-FB-01** | Zero-Secret OAuth & Single-Use PKCE Exchange | `PLANNED` | Task 1 (Step 1) | Desktop binary contains 0 secrets. Edge Function performs token exchange, issues single-use PKCE exchange code (60s TTL). User client invokes RPC deriving tenant exclusively via `auth.uid()`. Payload is deleted atomically upon retrieval. | Attacker attempts to claim exchange code without user JWT or with wrong PKCE verifier; RPC raises exception and returns 403. | Unit test: Call `/meta-oauth-exchange` with invalid verifier or forged user JWT; assert 403. |
| **REQ-FB-02** | Local Token Encryption at Rest | `PLANNED` | Task 3 (Step 1) | Page tokens encrypted with AES-256-GCM using OS Keyring master key before writing to SQLite `facebook_page_tokens`. | Reading `facebook_page_tokens.access_token_encrypted` directly yields ciphertext; decrypting without keyring key fails. | Cryptographic unit test: Verify ciphertext format and tamper resistance. |
| **REQ-FB-03** | Durable Webhook Ingestion & Raw HMAC Validation | `PLANNED` | Task 4 (Step 1) | Raw body verified against `META_APP_SECRET` using HMAC-SHA256. Unpacked events inserted with `ON CONFLICT DO NOTHING`. Durable interaction tracking in `meta_user_page_interactions`. | Ingestion failure returns HTTP 500 so Meta automatically retries; duplicate webhooks are silently ignored. | Integration test: Simulate duplicate webhook payload; assert exactly 1 row in `meta_webhook_buffer`. |
| **REQ-FB-04** | Contiguous Checkpointing, Out-of-Order Reactions & Receipts | `PLANNED` | Task 5 (Step 1) | Messenger sync worker processes events contiguously. Quarantines invalid events to `channel_sync_dead_letters`. Stores out-of-order reactions in `pending_message_reactions`. Populates `message_receipts` on delivery/read. Uses unique conflict target for provider attachments. Advances cursor monotonically per processed/quarantined event. | Malformed event followed by valid event: worker quarantines malformed event to dead-letters and advances cursor without skipping or halting. Out-of-order reaction arrives early; worker persists to pending reactions. | Integration test: Ingest malformed event and out-of-order reaction; verify dead-letter logging, pending reaction table, and monotonic cursor advancement. |
| **REQ-FB-05** | Production SQLite Migration (V1 -> V3) | `PLANNED` | Task 2 (Step 1) | `apply_migration_v3` handles schema transition with `SAVEPOINT`, exact state-restoring RAII `PragmaGuard`, foreign key validation, dynamic column backfills, and lossless metadata merge. | Migrate legacy DB with existing `channel_identities` and `message_attachments`; assert 0 data loss and foreign key integrity. | Unit test: `test_populated_production_v1_to_v3_migration_preserves_foreign_keys_and_dependents`. |
| **REQ-FB-06** | 24-Hour Interactive Countdown & Policy Gate | `PLANNED` | Task 6 (Step 1) | Outbound messages past 24h of customer's last message require valid `messaging_type=MESSAGE_TAG` and approved tag. | Attempting to send text message >24h without tag triggers modal blocking send until tag is selected. | UI test: Mock conversation >24h; verify message input disables free send and shows Tag selector. |
| **REQ-FB-07** | Outbox Sender with Typed Error Classifier | `PLANNED` | Task 6 (Step 1) | Outbox classifies errors into Transient (retry) and Permanent (dead-letter + UI prompt). | Error `2018278` (Outside 24h window) marks message `dead_letter` without wasteful retries. | Outbox unit test: Mock Meta API error responses; verify status transitions. |

---

### Task 1: Zero-Secret PKCE OAuth Relay & Edge Functions

**Files:**
- Create: `supabase/functions/meta-oauth-start/index.ts`
- Create: `supabase/functions/meta-oauth-callback/index.ts`
- Create: `supabase/functions/meta-oauth-exchange/index.ts`
- Create: `supabase/migrations/20260821000001_meta_oauth_tables.sql`

- [ ] **Step 1: Write Supabase Migration for OAuth Sessions and Single-Use PKCE Codes**

```sql
-- supabase/migrations/20260821000001_meta_oauth_tables.sql
CREATE TABLE IF NOT EXISTS meta_tenant_pages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    page_id TEXT NOT NULL,
    page_name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, page_id)
);
ALTER TABLE meta_tenant_pages ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Tenants read own pages" ON meta_tenant_pages
    FOR SELECT USING (auth.uid() = tenant_id);

CREATE TABLE IF NOT EXISTS oauth_authorization_sessions (
    state TEXT PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    code_challenge TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
ALTER TABLE oauth_authorization_sessions ENABLE ROW LEVEL SECURITY;

CREATE TABLE IF NOT EXISTS oauth_exchange_codes (
    code TEXT PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    code_challenge TEXT NOT NULL,
    payload TEXT NOT NULL,
    consumed BOOLEAN NOT NULL DEFAULT FALSE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
ALTER TABLE oauth_exchange_codes ENABLE ROW LEVEL SECURITY;

-- PostgreSQL Function for Atomic Consumption, Tenant Derivation and Deletion
CREATE OR REPLACE FUNCTION complete_meta_oauth_exchange(
    p_code TEXT,
    p_verifier_challenge TEXT
)
RETURNS JSONB
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, auth, pg_temp
AS $$
DECLARE
    v_tenant_id UUID;
    v_payload TEXT;
    v_pages JSONB;
    v_page RECORD;
BEGIN
    v_tenant_id := auth.uid();
    IF v_tenant_id IS NULL THEN
        RAISE EXCEPTION 'Unauthorized: user JWT required';
    END IF;

    SELECT payload INTO v_payload
    FROM public.oauth_exchange_codes
    WHERE code = p_code
      AND tenant_id = v_tenant_id
      AND code_challenge = p_verifier_challenge
      AND consumed = FALSE
      AND expires_at > now()
    FOR UPDATE;

    IF v_payload IS NULL THEN
        RAISE EXCEPTION 'Invalid or expired exchange code';
    END IF;

    -- Atomically consume and erase payload
    DELETE FROM public.oauth_exchange_codes WHERE code = p_code;

    v_pages := v_payload::JSONB;

    FOR v_page IN SELECT * FROM jsonb_to_recordset(v_pages) AS (id TEXT, name TEXT)
    LOOP
        INSERT INTO public.meta_tenant_pages (tenant_id, page_id, page_name)
        VALUES (v_tenant_id, v_page.id, v_page.name)
        ON CONFLICT (tenant_id, page_id) DO UPDATE SET page_name = EXCLUDED.page_name;
    END LOOP;

    RETURN v_pages;
END;
$$;

REVOKE ALL ON FUNCTION complete_meta_oauth_exchange FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION complete_meta_oauth_exchange TO authenticated, service_role;
```

- [ ] **Step 2: Implement `/meta-oauth-start` Edge Function**

```typescript
// supabase/functions/meta-oauth-start/index.ts
import { serve } from "https://deno.land/std@0.168.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2";

const SUPABASE_URL = Deno.env.get("SUPABASE_URL") || "";
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") || "";
const supabaseAdmin = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY);

serve(async (req: Request) => {
  if (req.method !== "POST") return new Response("Method not allowed", { status: 405 });
  const authHeader = req.headers.get("Authorization");
  if (!authHeader) return new Response("Unauthorized", { status: 401 });

  const supabaseClient = createClient(SUPABASE_URL, Deno.env.get("SUPABASE_ANON_KEY") || "", {
    global: { headers: { Authorization: authHeader } },
  });
  const { data: { user }, error: userError } = await supabaseClient.auth.getUser();
  if (userError || !user) return new Response("Unauthorized", { status: 401 });

  const { state, code_challenge } = await req.json();
  if (!state || !code_challenge) return new Response("Missing state or code_challenge", { status: 400 });

  const { error } = await supabaseAdmin.from("oauth_authorization_sessions").insert({
    state,
    tenant_id: user.id,
    code_challenge,
    expires_at: new Date(Date.now() + 10 * 60 * 1000).toISOString(),
  });

  if (error) return new Response("Database error", { status: 500 });
  return new Response(JSON.stringify({ status: "ok" }), { status: 200, headers: { "Content-Type": "application/json" } });
});
```

- [ ] **Step 3: Implement `/meta-oauth-callback` and `/meta-oauth-exchange` Edge Functions**

```typescript
// supabase/functions/meta-oauth-callback/index.ts
import { serve } from "https://deno.land/std@0.168.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2";

const META_APP_ID = Deno.env.get("META_APP_ID") || "";
const META_APP_SECRET = Deno.env.get("META_APP_SECRET") || "";
const SUPABASE_URL = Deno.env.get("SUPABASE_URL") || "";
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") || "";
const supabaseAdmin = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY);

serve(async (req: Request) => {
  const url = new URL(req.url);
  const code = url.searchParams.get("code");
  const state = url.searchParams.get("state");

  if (!code || !state) {
    return new Response("Missing code or state", { status: 400 });
  }

  // 1. Consume authorization session
  const { data: session, error: sessErr } = await supabaseAdmin
    .from("oauth_authorization_sessions")
    .delete()
    .eq("state", state)
    .gt("expires_at", new Date().toISOString())
    .select("tenant_id, code_challenge")
    .single();

  if (sessErr || !session) {
    return new Response("Invalid or expired OAuth session", { status: 403 });
  }

  // 2. Exchange code for short-lived token
  const redirectUri = `${SUPABASE_URL}/functions/v1/meta-oauth-callback`;
  const tokenUrl = `https://graph.facebook.com/v26.0/oauth/access_token?client_id=${META_APP_ID}&client_secret=${META_APP_SECRET}&redirect_uri=${encodeURIComponent(redirectUri)}&code=${encodeURIComponent(code)}`;
  const tokenResp = await fetch(tokenUrl);
  const tokenJson = await tokenResp.json();
  if (!tokenJson.access_token) {
    return new Response(`Token exchange failed: ${JSON.stringify(tokenJson)}`, { status: 502 });
  }

  // 3. Exchange for long-lived user token
  const longLivedUrl = `https://graph.facebook.com/v26.0/oauth/access_token?grant_type=fb_exchange_token&client_id=${META_APP_ID}&client_secret=${META_APP_SECRET}&fb_exchange_token=${tokenJson.access_token}`;
  const longResp = await fetch(longLivedUrl);
  const longJson = await longResp.json();
  const userToken = longJson.access_token || tokenJson.access_token;

  // 4. Fetch Pages
  const accountsUrl = `https://graph.facebook.com/v26.0/me/accounts?access_token=${userToken}`;
  const accResp = await fetch(accountsUrl);
  const accJson = await accResp.json();
  const pages = accJson.data || [];

  // 5. Store exchange code bound to tenant and PKCE challenge
  const exchangeCode = crypto.randomUUID();
  const { error: insErr } = await supabaseAdmin.from("oauth_exchange_codes").insert({
    code: exchangeCode,
    tenant_id: session.tenant_id,
    code_challenge: session.code_challenge,
    payload: JSON.stringify(pages),
    expires_at: new Date(Date.now() + 60 * 1000).toISOString(),
  });

  if (insErr) {
    return new Response("Database error saving exchange code", { status: 500 });
  }

  return Response.redirect(`rustsale://oauth/facebook/callback?code=${exchangeCode}&state=${state}`, 302);
});
```

```typescript
// supabase/functions/meta-oauth-exchange/index.ts
import { serve } from "https://deno.land/std@0.168.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2";

const SUPABASE_URL = Deno.env.get("SUPABASE_URL") || "";
const SUPABASE_ANON_KEY = Deno.env.get("SUPABASE_ANON_KEY") || "";

async function sha256Base64Url(str: string): Promise<string> {
  const buf = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(str));
  const bytes = new Uint8Array(buf);
  let binary = "";
  for (let i = 0; i < bytes.byteLength; i++) binary += String.fromCharCode(bytes[i]);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

serve(async (req: Request) => {
  if (req.method !== "POST") return new Response("Method not allowed", { status: 405 });
  const authHeader = req.headers.get("Authorization");
  if (!authHeader) return new Response("Unauthorized", { status: 401 });

  // Use user-JWT client so auth.uid() is resolved inside PostgreSQL RPC
  const userClient = createClient(SUPABASE_URL, SUPABASE_ANON_KEY, {
    global: { headers: { Authorization: authHeader } },
  });

  const { code, code_verifier } = await req.json();
  if (!code || !code_verifier) return new Response("Bad request", { status: 400 });

  const calculatedChallenge = await sha256Base64Url(code_verifier);

  const { data: pages, error: rpcErr } = await userClient.rpc("complete_meta_oauth_exchange", {
    p_code: code,
    p_verifier_challenge: calculatedChallenge,
  });

  if (rpcErr || !pages) {
    return new Response("Invalid, expired or unverified exchange code", { status: 403 });
  }

  return new Response(JSON.stringify({ pages }), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
});
```

- [ ] **Step 4: Commit Task 1**

```bash
git add supabase/
git commit -m "feat(supabase): implement zero-secret PKCE OAuth exchange with user JWT RPC invocation"
```

---

### Task 2: SQLite Schema Migration V2 & V3 with RAII PragmaGuard

**Files:**
- Modify: `src-tauri/src/conversation/schema.sql`
- Modify: `src-tauri/src/conversation/migration_v2.rs`
- Modify: `src-tauri/src/conversation/migration.rs`
- Test: `src-tauri/src/conversation/migration_v2_test.rs`

- [ ] **Step 1: Implement RAII PragmaGuard, Foreign Key Check, and Dynamic Column Migration in `migration_v2.rs`**
- [ ] **Step 2: Run Unit Tests for Migration**

```bash
cargo test --manifest-path src-tauri/Cargo.toml migration_v2_test
```

- [ ] **Step 3: Commit Task 2**

```bash
git add src-tauri/src/conversation/
git commit -m "feat(db): implement migration v2 and v3 with exact RAII PragmaGuard and foreign key check"
```

---

### Task 3: Local Token Vault & Stronghold OS Keyring Integration

**Files:**
- Create: `src-tauri/src/conversation/facebook/vault.rs`
- Test: `src-tauri/src/conversation/facebook/vault_test.rs`

- [ ] **Step 1: Implement AES-256-GCM Vault with Keyring Master Key**
- [ ] **Step 2: Run vault tests**
- [ ] **Step 3: Commit Task 3**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(security): implement AES-256-GCM vault with OS keyring integration"
```

---

### Task 4: Supabase Durable Webhook Relay & Raw HMAC Validation

**Files:**
- Create: `supabase/functions/meta-webhook/index.ts`
- Create: `supabase/migrations/20260821000002_meta_webhook_buffer.sql`

- [ ] **Step 1: Write Webhook Buffer DDL, RLS Policies, and Interaction Index**

```sql
-- supabase/migrations/20260821000002_meta_webhook_buffer.sql
CREATE TABLE IF NOT EXISTS meta_webhook_buffer (
    id BIGSERIAL PRIMARY KEY,
    page_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    event_key TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(page_id, event_key)
);
ALTER TABLE meta_webhook_buffer ENABLE ROW LEVEL SECURITY;

CREATE TABLE IF NOT EXISTS meta_user_page_interactions (
    page_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    last_interacted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(page_id, user_id)
);
ALTER TABLE meta_user_page_interactions ENABLE ROW LEVEL SECURITY;
```

- [ ] **Step 2: Implement `/meta-webhook` with HMAC validation**
- [ ] **Step 3: Commit Task 4**

```bash
git add supabase/
git commit -m "feat(webhook): implement durable webhook relay with HMAC verification and interaction index"
```

---

### Task 5: Inbound Facebook Messenger Sync Engine Matching Conversation Schema

**Files:**
- Create: `src-tauri/src/conversation/facebook/sync.rs`
- Create: `src-tauri/src/conversation/facebook/media_spool.rs`
- Create: `src-tauri/src/conversation/facebook/dead_letter_replay.rs`
- Test: `src-tauri/src/conversation/facebook/sync_test.rs`

- [ ] **Step 1: Implement Inbound Sync Worker with Contiguous Checkpointing, Out-of-Order Reaction Handling, and Receipt Ingestion**

```rust
// src-tauri/src/conversation/facebook/sync.rs
use anyhow::Result;
use rusqlite::{params, Connection};
use serde_json::Value;

pub struct FacebookSyncWorker;

impl FacebookSyncWorker {
    pub async fn pull_and_sync_messenger_events(
        conn: &mut Connection,
        channel_account_id: i64,
        events: &[Value],
    ) -> Result<i64> {
        let mut max_cursor = 0i64;

        for event in events {
            let event_id = event.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            let event_type = event.get("event_type").and_then(|v| v.as_str()).unwrap_or("");
            let event_key = event.get("event_key").and_then(|v| v.as_str()).unwrap_or("");
            let payload = event.get("payload").cloned().unwrap_or(Value::Null);

            let tx = conn.transaction()?;

            match event_type {
                "messages" | "message_echoes" => {
                    let is_echo = event_type == "message_echoes";
                    let msg_obj = payload.get("message");
                    let mid = msg_obj.and_then(|m| m.get("mid")).and_then(|m| m.as_str()).unwrap_or("").trim();
                    let sender_psid = payload.get("sender").and_then(|s| s.get("id")).and_then(|i| i.as_str()).unwrap_or("").trim();
                    let recipient_id = payload.get("recipient").and_then(|r| r.get("id")).and_then(|i| i.as_str()).unwrap_or("").trim();
                    let timestamp = payload.get("timestamp").and_then(|t| t.as_i64()).unwrap_or(0);
                    let psid = if is_echo { recipient_id } else { sender_psid };

                    // Strict identifier validation
                    if mid.is_empty() || psid.is_empty() || timestamp == 0 {
                        tx.execute(
                            "INSERT INTO channel_sync_dead_letters (channel_account_id, scope, event_id, item_index, event_type, event_key, payload_json, error_reason, status, created_at)
                             VALUES (?1, 'meta_webhook_messenger_cursor', ?2, 0, ?3, ?4, ?5, 'missing_required_identifier', 'failed', datetime('now'))
                             ON CONFLICT(channel_account_id, scope, event_id, item_index) DO NOTHING",
                            params![channel_account_id, event_id, event_type, event_key, serde_json::to_string(&payload)?],
                        )?;
                    } else {
                        let text = msg_obj.and_then(|m| m.get("text")).and_then(|t| t.as_str()).unwrap_or("");

                        // 1. Ensure channel_identities exists
                        tx.execute(
                            "INSERT INTO channel_identities (channel, channel_account_id, identity_namespace, external_id, created_at, updated_at)
                             VALUES ('messenger', ?1, 'psid', ?2, datetime('now'), datetime('now'))
                             ON CONFLICT(channel, channel_account_id, identity_namespace, external_id) WHERE channel_account_id IS NOT NULL DO UPDATE SET updated_at = datetime('now')",
                            params![channel_account_id, psid],
                        )?;

                        let identity_id: i64 = tx.query_row(
                            "SELECT id FROM channel_identities WHERE channel = 'messenger' AND channel_account_id = ?1 AND identity_namespace = 'psid' AND external_id = ?2",
                            params![channel_account_id, psid],
                            |r| r.get(0),
                        )?;

                        // 2. Ensure channel_conversations exists
                        tx.execute(
                            "INSERT INTO channel_conversations (channel_account_id, external_thread_id, kind, status, last_message_at, created_at, updated_at)
                             VALUES (?1, ?2, 'direct', 'active', datetime('now'), datetime('now'), datetime('now'))
                             ON CONFLICT(channel_account_id, external_thread_id, kind) DO UPDATE SET last_message_at = datetime('now'), updated_at = datetime('now')",
                            params![channel_account_id, psid],
                        )?;

                        let conversation_id: i64 = tx.query_row(
                            "SELECT id FROM channel_conversations WHERE channel_account_id = ?1 AND external_thread_id = ?2 AND kind = 'direct'",
                            params![channel_account_id, psid],
                            |r| r.get(0),
                        )?;

                        // 3. Ensure conversation_participants
                        tx.execute(
                            "INSERT INTO conversation_participants (conversation_id, identity_id, joined_at)
                             VALUES (?1, ?2, datetime('now'))
                             ON CONFLICT(conversation_id, identity_id) DO NOTHING",
                            params![conversation_id, identity_id],
                        )?;

                        // 4. Insert message record matching schema.sql
                        tx.execute(
                            "INSERT INTO messages (conversation_id, sender_identity_id, direction, provider_message_id, content_type, text_content, provider_timestamp, ingested_at, status)
                             VALUES (?1, ?2, ?3, ?4, 'text', ?5, ?6, datetime('now'), 'delivered')
                             ON CONFLICT(conversation_id, provider_message_id) WHERE provider_message_id IS NOT NULL DO NOTHING",
                            params![conversation_id, identity_id, if is_echo { "outbound" } else { "inbound" }, mid, text, timestamp.to_string()],
                        )?;

                        let message_id: i64 = tx.query_row(
                            "SELECT id FROM messages WHERE conversation_id = ?1 AND provider_message_id = ?2",
                            params![conversation_id, mid],
                            |r| r.get(0),
                        )?;

                        // 5. Process attachments with individual quarantine
                        if let Some(attachments) = msg_obj.and_then(|m| m.get("attachments")).and_then(|a| a.as_array()) {
                            for (idx, att) in attachments.iter().enumerate() {
                                let att_type = att.get("type").and_then(|t| t.as_str()).unwrap_or("file");
                                let payload_obj = att.get("payload");
                                let url = payload_obj.and_then(|p| p.get("url")).and_then(|u| u.as_str());
                                let provider_id = payload_obj.and_then(|p| p.get("attachment_id")).and_then(|id| id.as_str());

                                if let Some(remote_url) = url {
                                    tx.execute(
                                        "INSERT INTO message_attachments (message_id, provider_attachment_id, attachment_type, remote_url, metadata_json, created_at)
                                         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))
                                         ON CONFLICT(message_id, remote_url) WHERE remote_url IS NOT NULL DO NOTHING",
                                        params![message_id, provider_id, att_type, remote_url, serde_json::to_string(att)?],
                                    )?;
                                } else if let Some(pid) = provider_id {
                                    tx.execute(
                                        "INSERT INTO message_attachments (message_id, provider_attachment_id, attachment_type, metadata_json, created_at)
                                         VALUES (?1, ?2, ?3, ?4, datetime('now'))
                                         ON CONFLICT(message_id, provider_attachment_id) WHERE provider_attachment_id IS NOT NULL DO NOTHING",
                                        params![message_id, pid, att_type, serde_json::to_string(att)?],
                                    )?;
                                } else {
                                    tx.execute(
                                        "INSERT INTO channel_sync_dead_letters (channel_account_id, scope, event_id, item_index, event_type, event_key, payload_json, error_reason, status, created_at)
                                         VALUES (?1, 'meta_webhook_messenger_cursor', ?2, ?3, ?4, ?5, ?6, 'malformed_attachment_item', 'failed', datetime('now'))
                                         ON CONFLICT(channel_account_id, scope, event_id, item_index) DO NOTHING",
                                        params![channel_account_id, event_id, idx as i64, event_type, event_key, serde_json::to_string(&att)?],
                                    )?;
                                }
                            }
                        }

                        // 6. Check pending reactions for this message
                        let pending_reactions: Vec<(i64, String, String, i64)> = {
                            let mut stmt = tx.prepare(
                                "SELECT id, actor_external_id, reaction, removed FROM pending_message_reactions 
                                 WHERE channel_account_id = ?1 AND provider_message_id = ?2"
                            )?;
                            let rows = stmt.query_map(params![channel_account_id, mid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
                            rows.flatten().collect()
                        };

                        for (p_id, actor, emoji, removed) in pending_reactions {
                            if removed == 0 {
                                tx.execute(
                                    "INSERT INTO message_reactions (message_id, actor_external_id, reaction, count, reacted_at)
                                     VALUES (?1, ?2, ?3, 1, datetime('now'))
                                     ON CONFLICT(message_id, actor_external_id, reaction) DO UPDATE SET count = count + 1, reacted_at = datetime('now')",
                                    params![message_id, actor, emoji],
                                )?;
                            }
                            tx.execute("DELETE FROM pending_message_reactions WHERE id = ?1", [p_id])?;
                        }
                    }
                }
                "message_deliveries" => {
                    if let Some(del) = payload.get("delivery") {
                        let recipient = payload.get("recipient").and_then(|r| r.get("id")).and_then(|i| i.as_str()).unwrap_or("");
                        let watermark = del.get("watermark").and_then(|w| w.as_i64()).unwrap_or(0);
                        if let Some(mids) = del.get("mids").and_then(|m| m.as_array()) {
                            for mid in mids {
                                if let Some(m_str) = mid.as_str() {
                                    tx.execute("UPDATE messages SET status = 'delivered' WHERE provider_message_id = ?1", [m_str])?;
                                    if let Ok(msg_id) = tx.query_row("SELECT id FROM messages WHERE provider_message_id = ?1", [m_str], |r| r.get::<_, i64>(0)) {
                                        tx.execute(
                                            "INSERT INTO message_receipts (message_id, recipient_external_id, receipt_type, occurred_at, raw_payload_json)
                                             VALUES (?1, ?2, 'delivered', ?3, ?4)
                                             ON CONFLICT(message_id, recipient_external_id, receipt_type) DO NOTHING",
                                            params![msg_id, recipient, watermark.to_string(), serde_json::to_string(&payload)?],
                                        )?;
                                    }
                                }
                            }
                        }
                    }
                }
                "message_reads" => {
                    if let Some(watermark) = payload.get("read").and_then(|r| r.get("watermark")).and_then(|w| w.as_i64()) {
                        let recipient = payload.get("recipient").and_then(|r| r.get("id")).and_then(|i| i.as_str()).unwrap_or("");
                        let read_messages: Vec<i64> = {
                            let mut stmt = tx.prepare(
                                "SELECT id FROM messages 
                                 WHERE conversation_id IN (SELECT id FROM channel_conversations WHERE channel_account_id = ?1)
                                   AND provider_timestamp <= ?2"
                            )?;
                            let rows = stmt.query_map(params![channel_account_id, watermark.to_string()], |r| r.get(0))?;
                            rows.flatten().collect()
                        };
                        for msg_id in read_messages {
                            tx.execute("UPDATE messages SET status = 'read' WHERE id = ?1", [msg_id])?;
                            tx.execute(
                                "INSERT INTO message_receipts (message_id, recipient_external_id, receipt_type, occurred_at, raw_payload_json)
                                 VALUES (?1, ?2, 'read', ?3, ?4)
                                 ON CONFLICT(message_id, recipient_external_id, receipt_type) DO NOTHING",
                                params![msg_id, recipient, watermark.to_string(), serde_json::to_string(&payload)?],
                            )?;
                        }
                    }
                }
                "message_reactions" => {
                    if let Some(reaction_obj) = payload.get("reaction") {
                        let mid = reaction_obj.get("mid").and_then(|m| m.as_str()).unwrap_or("");
                        let action = reaction_obj.get("action").and_then(|a| a.as_str()).unwrap_or("react");
                        let emoji = reaction_obj.get("reaction").and_then(|r| r.as_str()).unwrap_or("");
                        let actor_id = payload.get("sender").and_then(|s| s.get("id")).and_then(|i| i.as_str()).unwrap_or("");
                        let psid = payload.get("recipient").and_then(|r| r.get("id")).and_then(|i| i.as_str()).unwrap_or("");

                        let msg_row: Result<i64, _> = tx.query_row(
                            "SELECT id FROM messages WHERE provider_message_id = ?1",
                            [mid],
                            |r| r.get(0),
                        );

                        match msg_row {
                            Ok(msg_id) => {
                                if action == "react" {
                                    tx.execute(
                                        "INSERT INTO message_reactions (message_id, actor_external_id, reaction, count, reacted_at)
                                         VALUES (?1, ?2, ?3, 1, datetime('now'))
                                         ON CONFLICT(message_id, actor_external_id, reaction) DO UPDATE SET count = count + 1, reacted_at = datetime('now')",
                                        params![msg_id, actor_id, emoji],
                                    )?;
                                } else {
                                    tx.execute(
                                        "DELETE FROM message_reactions WHERE message_id = ?1 AND actor_external_id = ?2 AND reaction = ?3",
                                        params![msg_id, actor_id, emoji],
                                    )?;
                                }
                            }
                            Err(_) => {
                                // Out-of-order reaction: save in pending_message_reactions
                                tx.execute(
                                    "INSERT INTO pending_message_reactions (
                                        channel_account_id, external_thread_id, conversation_kind, provider_event_id,
                                        provider_message_id, provider_client_message_id, actor_external_id, reaction, removed, occurred_at, raw_payload_json, created_at, updated_at
                                     ) VALUES (?1, ?2, 'direct', ?3, ?4, '', ?5, ?6, ?7, datetime('now'), ?8, datetime('now'), datetime('now'))
                                     ON CONFLICT DO NOTHING",
                                    params![channel_account_id, psid, event_id.to_string(), mid, actor_id, emoji, if action == "react" { 0 } else { 1 }, serde_json::to_string(&payload)?],
                                )?;
                            }
                        }
                    }
                }
                _ => {
                    tx.execute(
                        "INSERT INTO channel_sync_dead_letters (channel_account_id, scope, event_id, item_index, event_type, event_key, payload_json, error_reason, status, created_at)
                         VALUES (?1, 'meta_webhook_messenger_cursor', ?2, 0, ?3, ?4, ?5, 'unsupported_event_type', 'ignored', datetime('now'))
                         ON CONFLICT(channel_account_id, scope, event_id, item_index) DO NOTHING",
                        params![channel_account_id, event_id, event_type, event_key, serde_json::to_string(&payload)?],
                    )?;
                }
            }

            // Monotonically advance cursor after each event is processed or quarantined
            tx.execute(
                "INSERT INTO channel_sync_checkpoints (channel_account_id, scope, cursor, updated_at)
                 VALUES (?1, 'meta_webhook_messenger_cursor', ?2, datetime('now'))
                 ON CONFLICT(channel_account_id, scope) DO UPDATE SET cursor = ?2, updated_at = datetime('now')",
                params![channel_account_id, event_id.to_string()],
            )?;

            tx.commit()?;
            if event_id > max_cursor {
                max_cursor = event_id;
            }
        }

        Ok(max_cursor)
    }
}
```

- [ ] **Step 2: Commit Task 5**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(sync): implement contiguous messenger sync worker with receipt ingestion and out-of-order reaction queue"
```

---

### Task 6: Outbox Sender & 24-Hour Message Tag Policy Gate

**Files:**
- Create: `src-tauri/src/conversation/facebook/outbox.rs`
- Create: `src-tauri/src/conversation/facebook/error_classifier.rs`
- Test: `src-tauri/src/conversation/facebook/outbox_test.rs`

- [ ] **Step 1: Implement Outbox sender with Message Tag policy and error classification**
- [ ] **Step 2: Commit Task 6**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(outbox): implement outbox sender with 24h message tag policy gate"
```

---

### Task 7: Svelte 5 Unified Inbox Integration & Message Tag Selector

**Files:**
- Create: `src/lib/components/facebook/MessageTagSelector.svelte`
- Create: `src/lib/components/facebook/PolicyCountdownBadge.svelte`
- Modify: `src/lib/components/ConversationComposer.svelte`
- Modify: `src/lib/components/ConversationTimeline.svelte`

- [ ] **Step 1: Implement Svelte 5 PolicyCountdownBadge and MessageTagSelector**
- [ ] **Step 2: Run `bun run check`**
- [ ] **Step 3: Commit Task 7**

```bash
git add src/lib/components/
git commit -m "feat(ui): implement 24h policy countdown badge and message tag selector with svelte 5 runes"
```

---

### Task 8: Integration Verification & Gate Check

**Files:**
- Test: Full integration test suite

- [ ] **Step 1: Run Rust test suite**  
  `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] **Step 2: Run frontend build and linter**  
  `bun run check && bun run build`
- [ ] **Step 3: Final Commit for Phase 1**

```bash
git commit --allow-empty -m "chore: complete phase 1 facebook integration implementation"
```
