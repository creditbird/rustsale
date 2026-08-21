# Facebook Integration (Phase 1: Foundation, OAuth & Messenger 2-Way Sync) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish the production foundation for Facebook Integration: zero-secret PKCE OAuth token exchange with atomic session state machine and `pgp_sym_encrypt` payload handoff, encrypted local credential store, durable Supabase webhook relay buffer supporting messaging and changes (`feed`, `comments`, `leadgen`) with atomic server-side persistence RPC, isolated multi-stream sync engine for Messenger 2-way conversation matching repository schema (`channel_conversations`, `conversation_participants`, `messages`, `message_attachments`, `message_reactions`, `message_receipts`, `pending_message_reactions`), and Svelte 5 chat UI with 24-hour Message Tag policy enforcement.

**Tech Stack:** Tauri 2.0 (Rust), Svelte 5 (Runes), SQLite Local (`rusqlite`), Supabase Edge Functions (Deno / TypeScript), Meta Graph API v26.0+, Tailwind CSS.

---

## 1. Mandatory Implementation Harness Ledger (Tier 3)

| Req ID | Requirement | Status | Owning Step | Boundary Trace & Invariants | Adversarial Counterexample / Proof | Verification Method |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **REQ-FB-01** | Zero-Secret OAuth & Encrypted PKCE Exchange | `PLANNED` | Task 1 (Step 1) | Desktop binary contains 0 secrets. Edge Function performs token exchange, transitions session state atomically (`pending` -> `processing` -> `consumed`), encrypts payload with `pgp_sym_encrypt`, issues single-use PKCE exchange code (60s TTL). User client invokes RPC deriving tenant exclusively via `auth.uid()`. Payload is decrypted with `pgp_sym_decrypt` and deleted atomically upon retrieval. | Attacker attempts to claim exchange code without user JWT or with wrong PKCE verifier; RPC raises exception and returns 403. Reading Postgres WAL yields only ciphertext. | Unit test: Call `/meta-oauth-exchange` with invalid verifier or forged user JWT; assert 403. |
| **REQ-FB-02** | Local Token Encryption at Rest | `PLANNED` | Task 3 (Step 1) | Page tokens encrypted with AES-256-GCM using OS Keyring master key before writing to SQLite `facebook_page_tokens`. | Reading `facebook_page_tokens.access_token_encrypted` directly yields ciphertext; decrypting without keyring key fails. | Cryptographic unit test: Verify ciphertext format and tamper resistance. |
| **REQ-FB-03** | Durable Webhook Ingestion & Raw HMAC Validation | `PLANNED` | Task 4 (Step 1) | Raw body verified against `META_APP_SECRET` using HMAC-SHA256. Ingests both `entry.messaging` and `entry.changes` (feed, comments, leadgen). Generates stable deterministic event keys. Buffer and interaction index updated atomically via PostgreSQL RPC `ingest_meta_webhook_batch`. | Ingestion failure returns HTTP 500 so Meta automatically retries; duplicate webhooks are silently ignored. | Integration test: Simulate duplicate webhook payload; assert exactly 1 row in `meta_webhook_buffer`. |
| **REQ-FB-04** | Contiguous Checkpointing, Forward-Only State Machine & Receipts | `PLANNED` | Task 5 (Step 1) | Messenger sync worker processes events contiguously. Quarantines invalid events and unresolved dependencies to `channel_sync_dead_letters` with per-item index `item_index = idx as i64`. Scopes receipts and lookups to exact conversation and non-page customer PSID. Forward-only updates `messages.status = 'delivered'` without downgrading `read`. Marks only outbound messages read. Reconciles pending reactions ordered by `datetime(occurred_at) ASC, id ASC`. Advances cursor monotonically per processed/quarantined event via conditional update `WHERE CAST(excluded.cursor AS INTEGER) > CAST(COALESCE(cursor, '0') AS INTEGER)`. | Delayed delivery event arrives after read event: message remains `read`. Unresolved delivery MIDs each get distinct dead-letter rows. | Integration test: Ingest out-of-order delivery after read, duplicate reaction, and out-of-order reaction; verify state monotonicity, dead-letter logging, and monotonic cursor advancement. |
| **REQ-FB-05** | Production SQLite Migration (V1 -> V3) | `PLANNED` | Task 2 (Step 1) | `apply_migration_v3` handles schema transition with `SAVEPOINT`, exact state-restoring RAII `PragmaGuard`, foreign key validation, truly lossless Rust-level duplicate provider attachment and remote_url field and metadata merging (`json_patch`/map merge), and dynamic column backfills. | Migrate legacy DB with duplicate attachments containing distinct JSON keys; assert union of both keys is preserved. | Unit test: `test_populated_production_v1_to_v3_migration_preserves_foreign_keys_and_dependents`. |
| **REQ-FB-06** | 24-Hour Interactive Countdown & Policy Gate | `PLANNED` | Task 6 (Step 1) | Outbound messages past 24h of customer's last message require valid `messaging_type=MESSAGE_TAG` and approved tag. | Attempting to send text message >24h without tag triggers modal blocking send until tag is selected. | UI test: Mock conversation >24h; verify message input disables free send and shows Tag selector. |
| **REQ-FB-07** | Outbox Sender with Typed Error Classifier | `PLANNED` | Task 6 (Step 1) | Outbox classifies errors into Transient (retry) and Permanent (dead-letter + UI prompt). | Error `2018278` (Outside 24h window) marks message `dead_letter` without wasteful retries. | Outbox unit test: Mock Meta API error responses; verify status transitions. |

---

### Task 1: Zero-Secret PKCE OAuth Relay & Edge Functions

**Files:**
- Create: `supabase/functions/meta-oauth-start/index.ts`
- Create: `supabase/functions/meta-oauth-callback/index.ts`
- Create: `supabase/functions/meta-oauth-exchange/index.ts`
- Create: `supabase/migrations/20260821000001_meta_oauth_tables.sql`

- [ ] **Step 1: Write Supabase Migration for OAuth Sessions, Single-Use PKCE Codes, and Decryption RPC**

```sql
-- supabase/migrations/20260821000001_meta_oauth_tables.sql
CREATE EXTENSION IF NOT EXISTS pgcrypto;

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
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'consumed', 'failed')),
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
ALTER TABLE oauth_authorization_sessions ENABLE ROW LEVEL SECURITY;

CREATE TABLE IF NOT EXISTS oauth_exchange_codes (
    code TEXT PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    code_challenge TEXT NOT NULL,
    payload BYTEA NOT NULL,
    consumed BOOLEAN NOT NULL DEFAULT FALSE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
ALTER TABLE oauth_exchange_codes ENABLE ROW LEVEL SECURITY;

-- Helper RPC to create encrypted exchange code
CREATE OR REPLACE FUNCTION create_meta_oauth_exchange_code(
    p_code TEXT,
    p_tenant_id UUID,
    p_code_challenge TEXT,
    p_pages_json TEXT,
    p_encryption_key TEXT
)
RETURNS VOID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, auth, pg_temp
AS $$
BEGIN
    INSERT INTO public.oauth_exchange_codes (code, tenant_id, code_challenge, payload, expires_at)
    VALUES (
        p_code,
        p_tenant_id,
        p_code_challenge,
        pgp_sym_encrypt(p_pages_json, p_encryption_key),
        now() + INTERVAL '60 seconds'
    );
END;
$$;

REVOKE ALL ON FUNCTION create_meta_oauth_exchange_code FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION create_meta_oauth_exchange_code TO service_role;

-- PostgreSQL Function for Atomic Consumption, Tenant Derivation and Decryption
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
    v_payload_bytes BYTEA;
    v_pages JSONB;
    v_page RECORD;
BEGIN
    v_tenant_id := auth.uid();
    IF v_tenant_id IS NULL THEN
        RAISE EXCEPTION 'Unauthorized: user JWT required';
    END IF;

    SELECT payload INTO v_payload_bytes
    FROM public.oauth_exchange_codes
    WHERE code = p_code
      AND tenant_id = v_tenant_id
      AND code_challenge = p_verifier_challenge
      AND consumed = FALSE
      AND expires_at > now()
    FOR UPDATE;

    IF v_payload_bytes IS NULL THEN
        RAISE EXCEPTION 'Invalid or expired exchange code';
    END IF;

    -- Atomically consume and delete payload
    DELETE FROM public.oauth_exchange_codes WHERE code = p_code;

    -- Decrypt ciphertext using client PKCE challenge
    v_pages := pgp_sym_decrypt(v_payload_bytes, p_verifier_challenge)::JSONB;

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
    status: "pending",
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

  // 1. Atomically claim authorization session with state transition to 'processing'
  const { data: session, error: sessErr } = await supabaseAdmin
    .from("oauth_authorization_sessions")
    .update({ status: "processing", updated_at: new Date().toISOString() })
    .eq("state", state)
    .eq("status", "pending")
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
    await supabaseAdmin.from("oauth_authorization_sessions").update({ status: "failed" }).eq("state", state);
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

  // 5. Store exchange code encrypted with pgp_sym_encrypt using code_challenge as key
  const exchangeCode = crypto.randomUUID();
  const { error: insErr } = await supabaseAdmin.rpc("create_meta_oauth_exchange_code", {
    p_code: exchangeCode,
    p_tenant_id: session.tenant_id,
    p_code_challenge: session.code_challenge,
    p_pages_json: JSON.stringify(pages),
    p_encryption_key: session.code_challenge,
  });

  if (insErr) {
    await supabaseAdmin.from("oauth_authorization_sessions").update({ status: "failed" }).eq("state", state);
    return new Response("Database error saving exchange code", { status: 500 });
  }

  // Mark authorization session consumed
  await supabaseAdmin.from("oauth_authorization_sessions").update({ status: "consumed" }).eq("state", state);

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
git commit -m "feat(supabase): implement zero-secret PKCE OAuth exchange with atomic session state machine and pgp_sym_encrypt handoff"
```

---

### Task 2: SQLite Schema Migration V2 & V3 with RAII PragmaGuard

**Files:**
- Modify: `src-tauri/src/conversation/schema.sql`
- Modify: `src-tauri/src/conversation/migration_v2.rs`
- Modify: `src-tauri/src/conversation/migration.rs`
- Test: `src-tauri/src/conversation/migration_v2_test.rs`

- [ ] **Step 1: Implement RAII PragmaGuard, Foreign Key Check, and Truly Lossless Rust Duplicate Merging in `migration_v2.rs`**
- [ ] **Step 2: Run Unit Tests for Migration**

```bash
cargo test --manifest-path src-tauri/Cargo.toml migration_v2_test
```

- [ ] **Step 3: Commit Task 2**

```bash
git add src-tauri/src/conversation/
git commit -m "feat(db): implement migration v2 and v3 with exact RAII PragmaGuard, lossless Rust field and metadata merger, and foreign key check"
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

- [ ] **Step 1: Write Webhook Buffer DDL, RLS Policies, and Atomic Persistence RPC**

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
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    page_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    last_interacted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(tenant_id, page_id, user_id)
);
ALTER TABLE meta_user_page_interactions ENABLE ROW LEVEL SECURITY;

-- Atomic batch ingestion and interaction tracking RPC
CREATE OR REPLACE FUNCTION ingest_meta_webhook_batch(
    p_page_id TEXT,
    p_events JSONB
)
RETURNS VOID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, auth, pg_temp
AS $$
DECLARE
    v_evt RECORD;
    v_owner RECORD;
    v_user_id TEXT;
BEGIN
    FOR v_evt IN SELECT * FROM jsonb_to_recordset(p_events) AS (event_type TEXT, event_key TEXT, payload JSONB, customer_id TEXT)
    LOOP
        INSERT INTO public.meta_webhook_buffer (page_id, event_type, event_key, payload)
        VALUES (p_page_id, v_evt.event_type, v_evt.event_key, v_evt.payload)
        ON CONFLICT (page_id, event_key) DO NOTHING;

        IF v_evt.customer_id IS NOT NULL AND v_evt.customer_id != '' AND v_evt.customer_id != p_page_id THEN
            FOR v_owner IN SELECT tenant_id FROM public.meta_tenant_pages WHERE page_id = p_page_id
            LOOP
                INSERT INTO public.meta_user_page_interactions (tenant_id, page_id, user_id, last_interacted_at)
                VALUES (v_owner.tenant_id, p_page_id, v_evt.customer_id, now())
                ON CONFLICT (tenant_id, page_id, user_id) DO UPDATE SET last_interacted_at = now();
            END LOOP;
        END IF;
    END LOOP;
END;
$$;

REVOKE ALL ON FUNCTION ingest_meta_webhook_batch FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION ingest_meta_webhook_batch TO service_role;
```

- [ ] **Step 2: Implement `/meta-webhook` with HMAC validation, Stable Event Keys and Atomic RPC Ingestion**

```typescript
// supabase/functions/meta-webhook/index.ts
import { serve } from "https://deno.land/std@0.168.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2";

const META_APP_SECRET = Deno.env.get("META_APP_SECRET") || "";
const SUPABASE_URL = Deno.env.get("SUPABASE_URL") || "";
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") || "";
const supabaseAdmin = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY);

async function sha256Hex(str: string): Promise<string> {
  const buf = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(str));
  const bytes = new Uint8Array(buf);
  return Array.from(bytes).map(b => b.toString(16).padStart(2, "0")).join("");
}

serve(async (req: Request) => {
  const url = new URL(req.url);

  // Webhook verification handshake
  if (req.method === "GET") {
    const mode = url.searchParams.get("hub.mode");
    const token = url.searchParams.get("hub.verify_token");
    const challenge = url.searchParams.get("hub.challenge");
    if (mode === "subscribe" && token === Deno.env.get("META_VERIFY_TOKEN")) {
      return new Response(challenge, { status: 200 });
    }
    return new Response("Forbidden", { status: 403 });
  }

  if (req.method !== "POST") return new Response("Method not allowed", { status: 405 });

  const signature = req.headers.get("x-hub-signature-256");
  const rawBody = await req.text();

  if (!signature || !signature.startsWith("sha256=")) {
    return new Response("Missing signature", { status: 401 });
  }

  const sigHex = signature.slice(7);
  const matchBytes = sigHex.match(/.{1,2}/g);
  if (!matchBytes || matchBytes.length !== 32) {
    return new Response("Invalid signature format", { status: 403 });
  }

  const key = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(META_APP_SECRET),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["verify"]
  );

  const sigBytes = new Uint8Array(matchBytes.map(byte => parseInt(byte, 16)));
  const isValid = await crypto.subtle.verify("HMAC", key, sigBytes, new TextEncoder().encode(rawBody));

  if (!isValid) {
    return new Response("Invalid signature", { status: 403 });
  }

  let body;
  try {
    body = JSON.parse(rawBody);
  } catch {
    return new Response("Invalid JSON payload", { status: 400 });
  }

  if (body.object === "page") {
    for (const entry of body.entry || []) {
      const pageId = entry.id;
      const batchEvents: any[] = [];

      // 1. Ingest messaging events
      for (const messaging of entry.messaging || []) {
        const mid = messaging.message?.mid;
        const watermark = messaging.delivery?.watermark || messaging.read?.watermark;
        const reactionKey = messaging.reaction ? `${messaging.reaction.action}_${messaging.reaction.reaction}_${messaging.reaction.mid || ''}` : null;
        const senderId = messaging.sender?.id || "";
        const recipientId = messaging.recipient?.id || "";
        
        const eventType = messaging.message ? (messaging.message.is_echo ? "message_echoes" : "messages")
          : messaging.delivery ? "message_deliveries"
          : messaging.read ? "message_reads"
          : messaging.reaction ? "message_reactions" : "messaging_other";

        const eventKey = `${pageId}_${eventType}_${mid || watermark || reactionKey || messaging.timestamp}_${senderId}_${recipientId}_${messaging.timestamp}`;
        const customerId = (messaging.sender?.id === pageId) ? messaging.recipient?.id : messaging.sender?.id;

        batchEvents.push({
          event_type: eventType,
          event_key: eventKey,
          payload: messaging,
          customer_id: customerId,
        });
      }

      // 2. Ingest changes events (feed, comments, leadgen)
      for (const change of entry.changes || []) {
        const field = change.field || "change";
        const val = change.value || {};
        const verb = val.verb || "add";
        const targetId = val.comment_id || val.post_id || val.leadgen_id || "";
        const payloadHash = (await sha256Hex(JSON.stringify(val))).slice(0, 12);
        const changeKey = `${pageId}_${field}_${verb}_${targetId}_${val.created_time || entry.time || payloadHash}`;
        const eventType = field === "leadgen" ? "leadgen" : (field === "feed" || field === "comments" ? "feed" : "change_other");

        const bufferedPayload = field === "leadgen" ? {
          leadgen_id: val.leadgen_id,
          page_id: pageId,
          form_id: val.form_id,
          created_time: val.created_time,
          raw_change: change,
        } : change;

        const customerId = val.from?.id;

        batchEvents.push({
          event_type: eventType,
          event_key: changeKey,
          payload: bufferedPayload,
          customer_id: customerId,
        });
      }

      // Atomically persist buffer events and interaction index via Postgres RPC
      if (batchEvents.length > 0) {
        const { error: rpcErr } = await supabaseAdmin.rpc("ingest_meta_webhook_batch", {
          p_page_id: pageId,
          p_events: batchEvents,
        });

        if (rpcErr) {
          return new Response("Database error buffering events atomically", { status: 500 });
        }
      }
    }
  }

  return new Response("EVENT_RECEIVED", { status: 200 });
});
```

- [ ] **Step 3: Commit Task 4**

```bash
git add supabase/
git commit -m "feat(webhook): implement atomic server-side webhook ingestion RPC and stable change keys"
```

---

### Task 5: Inbound Facebook Messenger Sync Engine Matching Conversation Schema

**Files:**
- Create: `src-tauri/src/conversation/facebook/sync.rs`
- Create: `src-tauri/src/conversation/facebook/media_spool.rs`
- Create: `src-tauri/src/conversation/facebook/dead_letter_replay.rs`
- Test: `src-tauri/src/conversation/facebook/sync_test.rs`

- [ ] **Step 1: Implement Inbound Sync Worker with Contiguous Checkpointing, Forward-Only Status Updates, and Scoped Lookups**

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
        page_id: &str,
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

                        // 6. Check pending reactions for this message (reconciled in provider occurrence order)
                        let pending_reactions: Vec<(i64, String, String, i64)> = {
                            let mut stmt = tx.prepare(
                                "SELECT id, actor_external_id, reaction, removed FROM pending_message_reactions 
                                 WHERE channel_account_id = ?1 AND external_thread_id = ?2 AND provider_message_id = ?3
                                 ORDER BY datetime(occurred_at) ASC, id ASC"
                            )?;
                            let rows = stmt.query_map(params![channel_account_id, psid, mid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
                            rows.flatten().collect()
                        };

                        for (p_id, actor, emoji, removed) in pending_reactions {
                            if removed == 0 {
                                tx.execute(
                                    "INSERT INTO message_reactions (message_id, actor_external_id, reaction, count, reacted_at)
                                     VALUES (?1, ?2, ?3, 1, datetime('now'))
                                     ON CONFLICT(message_id, actor_external_id, reaction) DO UPDATE SET reacted_at = datetime('now')",
                                    params![message_id, actor, emoji],
                                )?;
                            } else {
                                tx.execute(
                                    "DELETE FROM message_reactions WHERE message_id = ?1 AND actor_external_id = ?2 AND reaction = ?3",
                                    params![message_id, actor, emoji],
                                )?;
                            }
                            tx.execute("DELETE FROM pending_message_reactions WHERE id = ?1", [p_id])?;
                        }
                    }
                }
                "message_deliveries" => {
                    let del = payload.get("delivery");
                    let recipient = payload.get("recipient").and_then(|r| r.get("id")).and_then(|i| i.as_str()).unwrap_or("").trim();
                    let sender = payload.get("sender").and_then(|s| s.get("id")).and_then(|i| i.as_str()).unwrap_or("").trim();
                    let psid = if recipient == page_id { sender } else { recipient };
                    let watermark = del.and_then(|d| d.get("watermark")).and_then(|w| w.as_i64()).unwrap_or(0);
                    let mids = del.and_then(|d| d.get("mids")).and_then(|m| m.as_array());

                    if psid.is_empty() || mids.is_none() {
                        tx.execute(
                            "INSERT INTO channel_sync_dead_letters (channel_account_id, scope, event_id, item_index, event_type, event_key, payload_json, error_reason, status, created_at)
                             VALUES (?1, 'meta_webhook_messenger_cursor', ?2, 0, ?3, ?4, ?5, 'malformed_delivery_event', 'failed', datetime('now'))
                             ON CONFLICT(channel_account_id, scope, event_id, item_index) DO NOTHING",
                            params![channel_account_id, event_id, event_type, event_key, serde_json::to_string(&payload)?],
                        )?;
                    } else {
                        let conversation_id_res: Result<i64, _> = tx.query_row(
                            "SELECT id FROM channel_conversations WHERE channel_account_id = ?1 AND external_thread_id = ?2 AND kind = 'direct'",
                            params![channel_account_id, psid],
                            |r| r.get(0),
                        );

                        match conversation_id_res {
                            Ok(conv_id) => {
                                for (idx, mid_val) in mids.unwrap().iter().enumerate() {
                                    if let Some(m_str) = mid_val.as_str() {
                                        // Forward-only transition: update delivered only if current status is sent, queued, or pending
                                        let updated_rows = tx.execute(
                                            "UPDATE messages SET status = 'delivered' WHERE conversation_id = ?1 AND provider_message_id = ?2 AND status IN ('sent', 'queued', 'pending')",
                                            params![conv_id, m_str],
                                        )?;
                                        if updated_rows > 0 {
                                            if let Ok(msg_id) = tx.query_row(
                                                "SELECT id FROM messages WHERE conversation_id = ?1 AND provider_message_id = ?2",
                                                params![conv_id, m_str],
                                                |r| r.get::<_, i64>(0),
                                            ) {
                                                tx.execute(
                                                    "INSERT INTO message_receipts (message_id, recipient_external_id, receipt_type, occurred_at, raw_payload_json)
                                                     VALUES (?1, ?2, 'delivered', ?3, ?4)
                                                     ON CONFLICT(message_id, recipient_external_id, receipt_type) DO NOTHING",
                                                    params![msg_id, psid, watermark.to_string(), serde_json::to_string(&payload)?],
                                                )?;
                                            }
                                        } else {
                                            // Message not yet ingested under existing conversation: record pending dead-letter with exact array index
                                            tx.execute(
                                                "INSERT INTO channel_sync_dead_letters (channel_account_id, scope, event_id, item_index, event_type, event_key, payload_json, error_reason, status, created_at)
                                                 VALUES (?1, 'meta_webhook_messenger_cursor', ?2, ?3, ?4, ?5, ?6, 'unresolved_message', 'pending', datetime('now'))
                                                 ON CONFLICT(channel_account_id, scope, event_id, item_index) DO NOTHING",
                                                params![channel_account_id, event_id, idx as i64, event_type, m_str, serde_json::to_string(&payload)?],
                                            )?;
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                // Conversation not yet synced: dead-letter with pending status for replay
                                tx.execute(
                                    "INSERT INTO channel_sync_dead_letters (channel_account_id, scope, event_id, item_index, event_type, event_key, payload_json, error_reason, status, created_at)
                                     VALUES (?1, 'meta_webhook_messenger_cursor', ?2, 0, ?3, ?4, ?5, 'unresolved_conversation', 'pending', datetime('now'))
                                     ON CONFLICT(channel_account_id, scope, event_id, item_index) DO NOTHING",
                                    params![channel_account_id, event_id, event_type, event_key, serde_json::to_string(&payload)?],
                                )?;
                            }
                        }
                    }
                }
                "message_reads" => {
                    let read_obj = payload.get("read");
                    let watermark = read_obj.and_then(|r| r.get("watermark")).and_then(|w| w.as_i64()).unwrap_or(0);
                    let recipient = payload.get("recipient").and_then(|r| r.get("id")).and_then(|i| i.as_str()).unwrap_or("").trim();
                    let sender = payload.get("sender").and_then(|s| s.get("id")).and_then(|i| i.as_str()).unwrap_or("").trim();
                    let psid = if recipient == page_id { sender } else { recipient };

                    if psid.is_empty() || watermark == 0 {
                        tx.execute(
                            "INSERT INTO channel_sync_dead_letters (channel_account_id, scope, event_id, item_index, event_type, event_key, payload_json, error_reason, status, created_at)
                             VALUES (?1, 'meta_webhook_messenger_cursor', ?2, 0, ?3, ?4, ?5, 'malformed_read_event', 'failed', datetime('now'))
                             ON CONFLICT(channel_account_id, scope, event_id, item_index) DO NOTHING",
                            params![channel_account_id, event_id, event_type, event_key, serde_json::to_string(&payload)?],
                        )?;
                    } else {
                        // Scope read receipts strictly to outbound messages in the direct conversation
                        let conversation_id_res: Result<i64, _> = tx.query_row(
                            "SELECT id FROM channel_conversations WHERE channel_account_id = ?1 AND external_thread_id = ?2 AND kind = 'direct'",
                            params![channel_account_id, psid],
                            |r| r.get(0),
                        );

                        match conversation_id_res {
                            Ok(conv_id) => {
                                let read_messages: Vec<i64> = {
                                    let mut stmt = tx.prepare(
                                        "SELECT id FROM messages 
                                         WHERE conversation_id = ?1 AND provider_timestamp <= ?2 AND direction = 'outbound'"
                                    )?;
                                    let rows = stmt.query_map(params![conv_id, watermark.to_string()], |r| r.get(0))?;
                                    rows.flatten().collect()
                                };
                                for msg_id in read_messages {
                                    tx.execute("UPDATE messages SET status = 'read' WHERE id = ?1", [msg_id])?;
                                    tx.execute(
                                        "INSERT INTO message_receipts (message_id, recipient_external_id, receipt_type, occurred_at, raw_payload_json)
                                         VALUES (?1, ?2, 'read', ?3, ?4)
                                         ON CONFLICT(message_id, recipient_external_id, receipt_type) DO NOTHING",
                                        params![msg_id, psid, watermark.to_string(), serde_json::to_string(&payload)?],
                                    )?;
                                }
                            }
                            Err(_) => {
                                tx.execute(
                                    "INSERT INTO channel_sync_dead_letters (channel_account_id, scope, event_id, item_index, event_type, event_key, payload_json, error_reason, status, created_at)
                                     VALUES (?1, 'meta_webhook_messenger_cursor', ?2, 0, ?3, ?4, ?5, 'unresolved_conversation', 'pending', datetime('now'))
                                     ON CONFLICT(channel_account_id, scope, event_id, item_index) DO NOTHING",
                                    params![channel_account_id, event_id, event_type, event_key, serde_json::to_string(&payload)?],
                                )?;
                            }
                        }
                    }
                }
                "message_reactions" => {
                    let reaction_obj = payload.get("reaction");
                    let mid = reaction_obj.and_then(|m| m.get("mid")).and_then(|m| m.as_str()).unwrap_or("").trim();
                    let action = reaction_obj.and_then(|a| a.get("action")).and_then(|a| a.as_str()).unwrap_or("react");
                    let emoji = reaction_obj.and_then(|r| r.get("reaction")).and_then(|r| r.as_str()).unwrap_or("").trim();
                    let actor_id = payload.get("sender").and_then(|s| s.get("id")).and_then(|i| i.as_str()).unwrap_or("").trim();
                    let recipient_id = payload.get("recipient").and_then(|r| r.get("id")).and_then(|i| i.as_str()).unwrap_or("").trim();
                    let psid = if actor_id == page_id { recipient_id } else { actor_id };
                    let timestamp = payload.get("timestamp").and_then(|t| t.as_i64()).unwrap_or(0);

                    if mid.is_empty() || actor_id.is_empty() || (action == "react" && emoji.is_empty()) || timestamp == 0 {
                        tx.execute(
                            "INSERT INTO channel_sync_dead_letters (channel_account_id, scope, event_id, item_index, event_type, event_key, payload_json, error_reason, status, created_at)
                             VALUES (?1, 'meta_webhook_messenger_cursor', ?2, 0, ?3, ?4, ?5, 'invalid_reaction_timestamp', 'failed', datetime('now'))
                             ON CONFLICT(channel_account_id, scope, event_id, item_index) DO NOTHING",
                            params![channel_account_id, event_id, event_type, event_key, serde_json::to_string(&payload)?],
                        )?;
                    } else {
                        let conversation_id_res: Result<i64, _> = tx.query_row(
                            "SELECT id FROM channel_conversations WHERE channel_account_id = ?1 AND external_thread_id = ?2 AND kind = 'direct'",
                            params![channel_account_id, psid],
                            |r| r.get(0),
                        );

                        let msg_row: Result<i64, _> = match conversation_id_res {
                            Ok(conv_id) => tx.query_row(
                                "SELECT id FROM messages WHERE conversation_id = ?1 AND provider_message_id = ?2",
                                params![conv_id, mid],
                                |r| r.get(0),
                            ),
                            Err(e) => Err(e),
                        };

                        match msg_row {
                            Ok(msg_id) => {
                                if action == "react" {
                                    tx.execute(
                                        "INSERT INTO message_reactions (message_id, actor_external_id, reaction, count, reacted_at)
                                         VALUES (?1, ?2, ?3, 1, datetime('now'))
                                         ON CONFLICT(message_id, actor_external_id, reaction) DO UPDATE SET reacted_at = datetime('now')",
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
                                // Out-of-order reaction: save in pending_message_reactions with provider occurrence timestamp string
                                let occurred_at_str = chrono::DateTime::from_timestamp_millis(timestamp)
                                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                                    .unwrap_or_else(|| "".to_string());

                                tx.execute(
                                    "INSERT INTO pending_message_reactions (
                                        channel_account_id, external_thread_id, conversation_kind, provider_event_id,
                                        provider_message_id, provider_client_message_id, actor_external_id, reaction, removed, occurred_at, raw_payload_json, created_at, updated_at
                                     ) VALUES (?1, ?2, 'direct', ?3, ?4, '', ?5, ?6, ?7, ?8, ?9, datetime('now'), datetime('now'))
                                     ON CONFLICT DO NOTHING",
                                    params![channel_account_id, psid, event_id.to_string(), mid, actor_id, emoji, if action == "react" { 0 } else { 1 }, occurred_at_str, serde_json::to_string(&payload)?],
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

            // Monotonically advance cursor strictly if event_id is greater than stored cursor
            tx.execute(
                "INSERT INTO channel_sync_checkpoints (channel_account_id, scope, cursor, updated_at)
                 VALUES (?1, 'meta_webhook_messenger_cursor', ?2, datetime('now'))
                 ON CONFLICT(channel_account_id, scope) DO UPDATE SET 
                     cursor = ?2,
                     updated_at = datetime('now')
                 WHERE CAST(excluded.cursor AS INTEGER) > CAST(COALESCE(channel_sync_checkpoints.cursor, '0') AS INTEGER)",
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
git commit -m "feat(sync): implement contiguous messenger sync worker with forward-only state transitions, array-indexed dead letters, and scoped receipts"
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
