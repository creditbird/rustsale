# Facebook Integration Phase 1 Implementation Plan: Foundation, Zero-Secret Auth, Durable Webhook Relay & Messenger 2-Way Sync

- **Goal:** Deliver production-ready Facebook Messenger integration with Zero-Secret PKCE OAuth, Server-Held Encryption at Rest, Local Vault Token Storage (AES-256-GCM + OS Keyring), Durable Webhook Ingestion Buffer with ACID Batch RPC, Canonical Deterministic JSON Hashing, Connected-Component Lossless Migration (V1 -> V4), Multi-Stream Isolated Cursors, Forward-Only Monotonic Message Status Updates, and Svelte 5 Unified Inbox with 24-Hour Policy Gate & Message Tag selector.
- **Architecture & Invariants:**
  - **Zero-Secret PKCE OAuth:** State machine with atomic 5-minute lease claims (`pending` / expired `processing` -> `processing`). Token payloads encrypted with server-held KMS/environment key `META_ENCRYPTION_KEY` via `pgp_sym_encrypt`. `complete_meta_oauth_exchange` derives tenant from `auth.uid()`, validates `SHA256(code_verifier) == code_challenge`, and decrypts using the server key.
  - **Durable Webhook Buffer & Ingestion:** Supabase Edge Function `/meta-webhook` verifies HMAC-SHA256 signature (`X-Hub-Signature-256`), calls PostgreSQL RPC `ingest_meta_webhook_batch` to atomically persist webhook events and upsert interaction mappings. Feed/leadgen change keys use canonical recursively-sorted JSON SHA-256 hashing.
  - **Connected-Component Lossless Migration:** V1 -> V4 migration executes graph connected-component deduplication on `message_attachments` across both `remote_url` and `provider_attachment_id`. Retains survivor `MIN(id)` with explicit survivor precedence, stores conflicting keys under `conflict_key_dup_<id>`, preserves unparsed values in `_legacy_unparsed_values`, and enforces `uq_message_attachments_remote` and `uq_message_attachments_provider`.
  - **Messenger Sync Engine:** Multi-stream isolation (`meta_webhook_messenger_cursor`, `meta_webhook_feed_cursor`, `meta_webhook_leadgen_cursor`). Monotonic forward-only status transitions (`sent`/`queued` -> `delivered`, `outbound` <= watermark -> `read`). Array-indexed dead-letter logging (`item_index = idx as i64`). Out-of-range reaction timestamps quarantined.

---

### Task 1: Supabase Backend Zero-Secret PKCE OAuth & Session State Machine

#### 1. SQL Migration (`supabase/migrations/20260821000001_meta_oauth.sql`)

```sql
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
    attempts INTEGER NOT NULL DEFAULT 0,
    lease_deadline TIMESTAMPTZ,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
ALTER TABLE oauth_authorization_sessions ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Tenants create own auth sessions" ON oauth_authorization_sessions
    FOR INSERT WITH CHECK (auth.uid() = tenant_id);

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

CREATE OR REPLACE FUNCTION create_meta_oauth_exchange_code(
    p_code TEXT,
    p_tenant_id UUID,
    p_code_challenge TEXT,
    p_pages_json TEXT
)
RETURNS VOID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, auth, pg_temp
AS $$
DECLARE
    v_server_key TEXT;
BEGIN
    -- Derive server-held encryption key from PostgreSQL configuration / vault (FAIL-CLOSED)
    v_server_key := current_setting('app.settings.meta_encryption_key', true);
    IF v_server_key IS NULL OR v_server_key = '' THEN
        RAISE EXCEPTION 'Missing configuration: app.settings.meta_encryption_key is required';
    END IF;

    INSERT INTO public.oauth_exchange_codes (code, tenant_id, code_challenge, payload, expires_at)
    VALUES (
        p_code,
        p_tenant_id,
        p_code_challenge,
        pgp_sym_encrypt(p_pages_json, v_server_key),
        now() + INTERVAL '60 seconds'
    );
END;
$$;

REVOKE ALL ON FUNCTION create_meta_oauth_exchange_code FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION create_meta_oauth_exchange_code TO service_role;

CREATE OR REPLACE FUNCTION complete_meta_oauth_exchange(
    p_code TEXT,
    p_code_verifier TEXT
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
    v_server_key TEXT;
    v_expected_challenge TEXT;
BEGIN
    v_tenant_id := auth.uid();
    IF v_tenant_id IS NULL THEN
        RAISE EXCEPTION 'Unauthorized: user JWT required';
    END IF;

    IF p_code_verifier IS NULL OR length(p_code_verifier) < 43 OR length(p_code_verifier) > 128 THEN
        RAISE EXCEPTION 'Invalid PKCE code_verifier length';
    END IF;

    -- Compute S256 challenge from verifier: Base64URL(SHA256(verifier))
    v_expected_challenge := rtrim(replace(replace(encode(digest(p_code_verifier, 'sha256'), 'base64'), '+', '-'), '/', '_'), '=');

    v_server_key := current_setting('app.settings.meta_encryption_key', true);
    IF v_server_key IS NULL OR v_server_key = '' THEN
        RAISE EXCEPTION 'Missing configuration: app.settings.meta_encryption_key is required';
    END IF;

    SELECT payload INTO v_payload_bytes
    FROM public.oauth_exchange_codes
    WHERE code = p_code
      AND tenant_id = v_tenant_id
      AND code_challenge = v_expected_challenge
      AND consumed = FALSE
      AND expires_at > now()
    FOR UPDATE;

    IF v_payload_bytes IS NULL THEN
        RAISE EXCEPTION 'Invalid, expired, or unverified exchange code';
    END IF;

    -- Atomically delete exchange code after retrieval
    DELETE FROM public.oauth_exchange_codes WHERE code = p_code;

    -- Decrypt payload using server-held key
    v_pages := pgp_sym_decrypt(v_payload_bytes, v_server_key)::JSONB;

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

#### 2. Supabase Edge Functions (`supabase/functions/meta-oauth/index.ts`)

```typescript
import { serve } from "https://deno.land/std@0.177.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2.39.8";

const SUPABASE_URL = Deno.env.get("SUPABASE_URL")!;
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY")!;
const META_APP_ID = Deno.env.get("META_APP_ID")!;
const META_APP_SECRET = Deno.env.get("META_APP_SECRET")!;

const supabaseAdmin = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY);

serve(async (req: Request) => {
  const url = new URL(req.url);

  if (url.pathname === "/meta-oauth-start") {
    const authHeader = req.headers.get("Authorization");
    if (!authHeader) return new Response("Unauthorized", { status: 401 });

    const userClient = createClient(SUPABASE_URL, authHeader.replace("Bearer ", ""), {
      global: { headers: { Authorization: authHeader } },
    });
    const { data: { user }, error: userError } = await userClient.auth.getUser();
    if (userError || !user) return new Response("Unauthorized", { status: 401 });

    const { state, code_challenge } = await req.json();
    if (!state || !code_challenge) return new Response("Missing parameters", { status: 400 });

    const { error: sessionError } = await supabaseAdmin
      .from("oauth_authorization_sessions")
      .insert({
        state,
        tenant_id: user.id,
        code_challenge,
        status: "pending",
        expires_at: new Date(Date.now() + 10 * 60 * 1000).toISOString(),
      });

    if (sessionError) return new Response("Session creation error", { status: 500 });

    const redirectUri = `${url.origin}/meta-oauth-callback`;
    const metaAuthUrl = `https://www.facebook.com/v26.0/dialog/oauth?client_id=${META_APP_ID}&redirect_uri=${encodeURIComponent(redirectUri)}&state=${encodeURIComponent(state)}&scope=pages_show_list,pages_read_engagement,pages_manage_posts,pages_messaging,leads_retrieval`;

    return new Response(JSON.stringify({ auth_url: metaAuthUrl }), {
      headers: { "Content-Type": "application/json" },
    });
  }

  if (url.pathname === "/meta-oauth-callback") {
    const code = url.searchParams.get("code");
    const state = url.searchParams.get("state");
    const errorParam = url.searchParams.get("error");

    if (errorParam || !code || !state) {
      if (state) {
        await supabaseAdmin
          .from("oauth_authorization_sessions")
          .update({ status: "failed", updated_at: new Date().toISOString() })
          .eq("state", state);
      }
      return new Response("OAuth consent canceled or failed", { status: 400 });
    }

    // Atomic session claim with 5-minute lease deadline
    const { data: claimedSession, error: claimError } = await supabaseAdmin
      .from("oauth_authorization_sessions")
      .update({
        status: "processing",
        attempts: 1,
        lease_deadline: new Date(Date.now() + 5 * 60 * 1000).toISOString(),
        updated_at: new Date().toISOString(),
      })
      .eq("state", state)
      .or("status.eq.pending,and(status.eq.processing,lease_deadline.lt.now())")
      .gt("expires_at", new Date().toISOString())
      .select("tenant_id, code_challenge")
      .single();

    if (claimError || !claimedSession) {
      return new Response("Invalid, expired, or already claimed session state", { status: 400 });
    }

    try {
      const redirectUri = `${url.origin}/meta-oauth-callback`;
      const tokenUrl = `https://graph.facebook.com/v26.0/oauth/access_token?client_id=${META_APP_ID}&redirect_uri=${encodeURIComponent(redirectUri)}&client_secret=${META_APP_SECRET}&code=${encodeURIComponent(code)}`;
      const tokenResp = await fetch(tokenUrl);
      if (!tokenResp.ok) throw new Error(`Token exchange failed: ${tokenResp.status}`);
      const tokenData = await tokenResp.json();
      const shortToken = tokenData.access_token;

      const longTokenUrl = `https://graph.facebook.com/v26.0/oauth/access_token?grant_type=fb_exchange_token&client_id=${META_APP_ID}&client_secret=${META_APP_SECRET}&fb_exchange_token=${encodeURIComponent(shortToken)}`;
      const longResp = await fetch(longTokenUrl);
      if (!longResp.ok) throw new Error(`Long lived token exchange failed: ${longResp.status}`);
      const longData = await longResp.json();
      const longLivedUserToken = longData.access_token;

      // Fail-closed ASID resolution from /me
      const meUrl = `https://graph.facebook.com/v26.0/me?access_token=${encodeURIComponent(longLivedUserToken)}`;
      const meResp = await fetch(meUrl);
      if (!meResp.ok) throw new Error(`Failed to resolve user identity from Meta Graph API: ${meResp.status}`);
      const meData = await meResp.json();
      const appUserId = meData.id;
      if (!appUserId) throw new Error("Meta user identity ID missing from /me response");

      const accountsUrl = `https://graph.facebook.com/v26.0/me/accounts?access_token=${encodeURIComponent(longLivedUserToken)}`;
      const accResp = await fetch(accountsUrl);
      if (!accResp.ok) throw new Error(`Accounts retrieval failed: ${accResp.status}`);
      const accData = await accResp.json();
      const pages = (accData.data || []).map((p: any) => ({
        id: p.id,
        name: p.name,
        access_token: p.access_token,
        category: p.category,
      }));

      // Fail-closed PSID resolution for each managed page via ids_for_pages
      const idsUrl = `https://graph.facebook.com/v26.0/${appUserId}/ids_for_pages?access_token=${encodeURIComponent(longLivedUserToken)}`;
      const idsResp = await fetch(idsUrl);
      if (!idsResp.ok) throw new Error(`Failed to resolve page-scoped identities from Meta Graph API: ${idsResp.status}`);
      const idsData = await idsResp.json();

      // Persist authoritative ASID-to-PSID mapping from Meta ids_for_pages
      const mappedPageIds = new Set<string>();
      for (const item of idsData.data || []) {
        if (item.page?.id && item.id) {
          const { error: upsertErr } = await supabaseAdmin.from("meta_user_page_interactions").upsert({
            tenant_id: claimedSession.tenant_id,
            page_id: item.page.id,
            app_user_id: appUserId,
            page_scoped_user_id: item.id,
            last_interaction_at: new Date().toISOString(),
          }, { onConflict: "tenant_id,page_id,page_scoped_user_id" });
          if (upsertErr) throw new Error(`Failed to persist durable user page interaction mapping: ${upsertErr.message}`);
          mappedPageIds.add(item.page.id);
        }
      }

      // Enforce 100% complete coverage: every connected page MUST have an authoritative ASID-to-PSID mapping
      const unmappedPages = pages.filter((p: any) => !mappedPageIds.has(p.id));
      if (unmappedPages.length > 0) {
        throw new Error(`Cannot connect pages: Meta Graph API /ids_for_pages did not return PSID mappings for connected page(s): ${unmappedPages.map((p: any) => p.id).join(", ")}. Every connected page requires an authoritative PSID mapping.`);
      }

      const exchangeCode = crypto.randomUUID();
      const { error: rpcError } = await supabaseAdmin.rpc("create_meta_oauth_exchange_code", {
        p_code: exchangeCode,
        p_tenant_id: claimedSession.tenant_id,
        p_code_challenge: claimedSession.code_challenge,
        p_pages_json: JSON.stringify(pages),
      });

      if (rpcError) throw rpcError;

      await supabaseAdmin
        .from("oauth_authorization_sessions")
        .update({ status: "consumed", updated_at: new Date().toISOString() })
        .eq("state", state);

      const desktopCallback = `rustsale://meta-oauth?code=${encodeURIComponent(exchangeCode)}&state=${encodeURIComponent(state)}`;
      return new Response(`<!DOCTYPE html><html><body><script>window.location.href = "${desktopCallback}";</script><p>Redirecting to RustSale Desktop...</p></body></html>`, {
        headers: { "Content-Type": "text/html" },
      });
    } catch (err) {
      await supabaseAdmin
        .from("oauth_authorization_sessions")
        .update({ status: "failed", updated_at: new Date().toISOString() })
        .eq("state", state);
      return new Response("OAuth processing error: " + String(err), { status: 500 });
    }
  }

  return new Response("Not Found", { status: 404 });
});
```

---

### Task 2: Durable Webhook Relay & Canonical Ingestion RPC

#### 1. SQL Migration (`supabase/migrations/20260821000002_meta_webhooks.sql`)

```sql
CREATE TABLE IF NOT EXISTS meta_webhook_buffer (
    id BIGSERIAL PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    page_id TEXT NOT NULL,
    field TEXT NOT NULL,
    event_key TEXT NOT NULL,
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, page_id, event_key)
);
ALTER TABLE meta_webhook_buffer ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE meta_webhook_buffer FROM PUBLIC, anon, authenticated;
GRANT SELECT, UPDATE(status) ON TABLE meta_webhook_buffer TO authenticated;
GRANT ALL ON TABLE meta_webhook_buffer TO service_role;

CREATE POLICY "Tenants select own buffered events" ON meta_webhook_buffer
    FOR SELECT TO authenticated
    USING (auth.uid() = tenant_id);

CREATE POLICY "Tenants ack own buffered events" ON meta_webhook_buffer
    FOR UPDATE TO authenticated
    USING (auth.uid() = tenant_id)
    WITH CHECK (auth.uid() = tenant_id AND status IN ('processing', 'completed', 'failed'));

CREATE TABLE IF NOT EXISTS meta_user_page_interactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    page_id TEXT NOT NULL,
    app_user_id TEXT,
    page_scoped_user_id TEXT NOT NULL,
    last_interaction_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, page_id, page_scoped_user_id)
);
CREATE INDEX IF NOT EXISTS idx_meta_interactions_asid ON meta_user_page_interactions(tenant_id, app_user_id);
CREATE INDEX IF NOT EXISTS idx_meta_interactions_psid ON meta_user_page_interactions(tenant_id, page_scoped_user_id);
ALTER TABLE meta_user_page_interactions ENABLE ROW LEVEL SECURITY;

CREATE TABLE IF NOT EXISTS meta_lead_page_interactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    page_id TEXT NOT NULL,
    leadgen_id TEXT NOT NULL,
    form_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, page_id, leadgen_id)
);
ALTER TABLE meta_lead_page_interactions ENABLE ROW LEVEL SECURITY;

CREATE OR REPLACE FUNCTION ingest_meta_webhook_batch(
    p_tenant_id UUID,
    p_page_id TEXT,
    p_events JSONB,
    p_interactions JSONB,
    p_lead_interactions JSONB DEFAULT '[]'::JSONB
)
RETURNS INTEGER
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, auth, pg_temp
AS $$
DECLARE
    v_inserted_count INTEGER := 0;
    v_event RECORD;
    v_inter RECORD;
    v_lead RECORD;
BEGIN
    FOR v_event IN SELECT * FROM jsonb_to_recordset(p_events) AS (field TEXT, event_key TEXT, payload JSONB)
    LOOP
        INSERT INTO public.meta_webhook_buffer (tenant_id, page_id, field, event_key, payload)
        VALUES (p_tenant_id, p_page_id, v_event.field, v_event.event_key, v_event.payload)
        ON CONFLICT (tenant_id, page_id, event_key) DO NOTHING;
        IF FOUND THEN
            v_inserted_count := v_inserted_count + 1;
        END IF;
    END LOOP;

    FOR v_inter IN SELECT * FROM jsonb_to_recordset(p_interactions) AS (user_id TEXT, app_user_id TEXT, timestamp TIMESTAMPTZ)
    LOOP
        INSERT INTO public.meta_user_page_interactions (tenant_id, page_id, app_user_id, page_scoped_user_id, last_interaction_at)
        VALUES (p_tenant_id, p_page_id, v_inter.app_user_id, v_inter.user_id, v_inter.timestamp)
        ON CONFLICT (tenant_id, page_id, page_scoped_user_id)
        DO UPDATE SET
            app_user_id = COALESCE(EXCLUDED.app_user_id, meta_user_page_interactions.app_user_id),
            last_interaction_at = GREATEST(meta_user_page_interactions.last_interaction_at, EXCLUDED.last_interaction_at);
    END LOOP;

    FOR v_lead IN SELECT * FROM jsonb_to_recordset(p_lead_interactions) AS (leadgen_id TEXT, form_id TEXT, created_at TIMESTAMPTZ)
    LOOP
        INSERT INTO public.meta_lead_page_interactions (tenant_id, page_id, leadgen_id, form_id, created_at)
        VALUES (p_tenant_id, p_page_id, v_lead.leadgen_id, v_lead.form_id, v_lead.created_at)
        ON CONFLICT (tenant_id, page_id, leadgen_id) DO NOTHING;
    END LOOP;

    RETURN v_inserted_count;
END;
$$;

REVOKE ALL ON FUNCTION ingest_meta_webhook_batch FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION ingest_meta_webhook_batch TO service_role;
```

#### 2. Webhook Edge Function (`supabase/functions/meta-webhook/index.ts`)

```typescript
import { serve } from "https://deno.land/std@0.177.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2.39.8";

const SUPABASE_URL = Deno.env.get("SUPABASE_URL")!;
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY")!;
const META_APP_SECRET = Deno.env.get("META_APP_SECRET")!;
const META_VERIFY_TOKEN = Deno.env.get("META_VERIFY_TOKEN")!;

const supabaseAdmin = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY);

function canonicalJsonStringify(obj: any): string {
  if (obj === null || typeof obj !== "object") {
    return JSON.stringify(obj);
  }
  if (Array.isArray(obj)) {
    return "[" + obj.map(canonicalJsonStringify).join(",") + "]";
  }
  const keys = Object.keys(obj).sort();
  const pairs = keys.map((k) => JSON.stringify(k) + ":" + canonicalJsonStringify(obj[k]));
  return "{" + pairs.join(",") + "}";
}

async function computeSha256(str: string): Promise<string> {
  const encoder = new TextEncoder();
  const data = encoder.encode(str);
  const hash = await crypto.subtle.digest("SHA-256", data);
  return Array.from(new Uint8Array(hash)).map((b) => b.toString(16).padStart(2, "0")).join("");
}

async function verifyMetaSignature(req: Request, rawBody: string): Promise<boolean> {
  const signatureHeader = req.headers.get("X-Hub-Signature-256");
  if (!signatureHeader || !signatureHeader.startsWith("sha256=")) return false;

  const expectedSignature = signatureHeader.replace("sha256=", "");
  const encoder = new TextEncoder();
  const keyData = encoder.encode(META_APP_SECRET);
  const key = await crypto.subtle.importKey("raw", keyData, { name: "HMAC", hash: "SHA-256" }, false, ["verify"]);
  const signatureBytes = new Uint8Array(
    expectedSignature.match(/.{1,2}/g)?.map((byte) => parseInt(byte, 16)) || []
  );

  return await crypto.subtle.verify("HMAC", key, signatureBytes, encoder.encode(rawBody));
}

serve(async (req: Request) => {
  const url = new URL(req.url);

  if (req.method === "GET") {
    const mode = url.searchParams.get("hub.mode");
    const token = url.searchParams.get("hub.verify_token");
    const challenge = url.searchParams.get("hub.challenge");

    if (mode === "subscribe" && token === META_VERIFY_TOKEN) {
      return new Response(challenge, { status: 200 });
    }
    return new Response("Forbidden", { status: 403 });
  }

  if (req.method === "POST") {
    const rawBody = await req.text();
    const isValid = await verifyMetaSignature(req, rawBody);
    if (!isValid) return new Response("Invalid signature", { status: 401 });

    const payload = JSON.parse(rawBody);
    if (payload.object !== "page") return new Response("Ignored non-page object", { status: 200 });

    for (const entry of payload.entry || []) {
      const pageId = entry.id;
      const { data: pageMappings, error: pageMappingError } = await supabaseAdmin
        .from("meta_tenant_pages")
        .select("tenant_id")
        .eq("page_id", pageId);

      if (pageMappingError) {
        console.error("Failed to query meta_tenant_pages mapping:", pageMappingError);
        return new Response("Internal Server Error: Page mapping query failed", { status: 500 });
      }

      if (!pageMappings || pageMappings.length === 0) continue;

      const events: any[] = [];
      const interactions: any[] = [];
      const leadInteractions: any[] = [];

      if (entry.messaging) {
        for (const msg of entry.messaging) {
          const senderId = msg.sender?.id;
          const timestamp = msg.timestamp ? new Date(msg.timestamp).toISOString() : new Date().toISOString();
          if (senderId && senderId !== pageId) {
            interactions.push({ user_id: senderId, app_user_id: null, timestamp });
          }

          let eventKey = "";
          if (msg.message?.mid) eventKey = `msg_${msg.message.mid}`;
          else if (msg.delivery?.mids) eventKey = `del_${msg.delivery.mids.join("_")}_${msg.delivery.watermark}`;
          else if (msg.read?.watermark) eventKey = `read_${senderId}_${msg.read.watermark}`;
          else if (msg.reaction?.mid) eventKey = `react_${msg.reaction.mid}_${senderId}_${msg.reaction.reaction || "none"}`;
          else eventKey = `misc_${senderId}_${await computeSha256(canonicalJsonStringify(msg))}`;

          events.push({ field: "messages", event_key: eventKey, payload: msg });
        }
      }

      if (entry.changes) {
        for (const ch of entry.changes) {
          const val = ch.value || {};
          const field = ch.field;
          const fromId = val.from?.id;
          const timestamp = val.created_time ? new Date(val.created_time * 1000).toISOString() : new Date().toISOString();

          if (fromId && fromId !== pageId) {
            interactions.push({ user_id: fromId, timestamp });
          }

          if (field === "leadgen" && val.leadgen_id) {
            leadInteractions.push({
              leadgen_id: val.leadgen_id,
              form_id: val.form_id || null,
              created_at: timestamp,
            });
          }

          const payloadHash = await computeSha256(canonicalJsonStringify(val));
          let eventKey = "";
          if (val.comment_id) eventKey = `comment_${val.comment_id}_${val.verb || "edit"}_${payloadHash}`;
          else if (val.post_id) eventKey = `post_${val.post_id}_${val.verb || "edit"}_${payloadHash}`;
          else if (val.leadgen_id) eventKey = `leadgen_${val.leadgen_id}_${payloadHash}`;
          else eventKey = `change_${field}_${payloadHash}`;

          events.push({ field, event_key: eventKey, payload: ch });
        }
      }

      if (events.length > 0) {
        for (const mapping of pageMappings) {
          const { error: rpcError } = await supabaseAdmin.rpc("ingest_meta_webhook_batch", {
            p_tenant_id: mapping.tenant_id,
            p_page_id: pageId,
            p_events: events,
            p_interactions: interactions,
            p_lead_interactions: leadInteractions,
          });

          if (rpcError) {
            console.error("Failed to ingest webhook batch for tenant:", mapping.tenant_id, rpcError);
            return new Response("Internal Server Error: Database ingestion failed", { status: 500 });
          }
        }
      }
    }

    return new Response("OK", { status: 200 });
  }

  return new Response("Method Not Allowed", { status: 405 });
});
```

---

### Task 3: Local SQLite Schema, V4 Migration & Rust Messenger Sync Engine

#### 1. Facebook Token Vault (`src-tauri/src/facebook/vault.rs`)
- Implements secure local credential storage backed by OS Keyring and AES-256-GCM authenticated encryption.
- Direct write path: After receiving OAuth exchange code via `rustsale://meta-oauth?code=...`, desktop calls `complete_meta_oauth_exchange` and iterates the returned pages, invoking `FacebookTokenVault::encrypt_and_upsert_page_token` to persist encrypted Page tokens.

```rust
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::prelude::*;
use keyring::Entry;
use rusqlite::{params, Connection, OptionalExtension};
use anyhow::{anyhow, Result};

pub struct FacebookTokenVault;

impl FacebookTokenVault {
    fn get_master_key() -> Result<[u8; 32]> {
        let entry = Entry::new("rustsale", "facebook_vault_key")
            .map_err(|e| anyhow!("Failed to access OS keyring: {}", e))?;
        match entry.get_password() {
            Ok(secret) => {
                let mut key = [0u8; 32];
                let decoded = BASE64_STANDARD.decode(secret.trim())
                    .map_err(|e| anyhow!("Invalid base64 in keyring secret: {}", e))?;
                if decoded.len() != 32 {
                    return Err(anyhow!("Keyring master key length is not 32 bytes"));
                }
                key.copy_from_slice(&decoded);
                Ok(key)
            }
            Err(keyring::Error::NoEntry) => {
                let mut key = [0u8; 32];
                rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut key);
                let encoded = BASE64_STANDARD.encode(key);
                entry.set_password(&encoded)
                    .map_err(|e| anyhow!("Failed to persist master key to OS keyring: {}", e))?;
                Ok(key)
            }
            Err(e) => Err(anyhow!("Keyring access failed: {}", e)),
        }
    }

    pub fn encrypt_token(plain_token: &str) -> Result<String> {
        let key = Self::get_master_key()?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| anyhow!("AES-256-GCM init error: {}", e))?;
        let mut nonce_bytes = [0u8; 12];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, plain_token.as_bytes())
            .map_err(|e| anyhow!("Token encryption error: {}", e))?;
        let mut combined = Vec::with_capacity(12 + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);
        Ok(BASE64_STANDARD.encode(combined))
    }

    pub fn decrypt_token(ciphertext_b64: &str) -> Result<String> {
        let raw = BASE64_STANDARD.decode(ciphertext_b64.trim())
            .map_err(|e| anyhow!("Base64 decode failed: {}", e))?;
        if raw.len() < 12 {
            return Err(anyhow!("Ciphertext payload too short for nonce"));
        }
        let (nonce_bytes, encrypted_payload) = raw.split_at(12);
        let key = Self::get_master_key()?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| anyhow!("AES-256-GCM cipher init error: {}", e))?;
        let nonce = Nonce::from_slice(nonce_bytes);
        let decrypted_bytes = cipher.decrypt(nonce, encrypted_payload)
            .map_err(|e| anyhow!("AES-256-GCM decryption failed: {}", e))?;
        String::from_utf8(decrypted_bytes)
            .map_err(|e| anyhow!("Decrypted token is not valid UTF-8: {}", e))
    }

    pub fn encrypt_and_upsert_page_token(
        conn: &Connection,
        channel_account_id: i64,
        page_id: &str,
        plain_token: &str,
        granted_scopes: &[String],
    ) -> Result<()> {
        let encrypted_ciphertext = Self::encrypt_token(plain_token)?;
        let scopes_json = serde_json::to_string(granted_scopes).unwrap_or_else(|_| "[]".to_string());
        conn.execute(
            "INSERT INTO facebook_page_tokens (channel_account_id, page_id, access_token_encrypted, token_status, granted_scopes_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'valid', ?4, datetime('now'), datetime('now'))
             ON CONFLICT(page_id) DO UPDATE SET
                 channel_account_id = EXCLUDED.channel_account_id,
                 access_token_encrypted = EXCLUDED.access_token_encrypted,
                 token_status = 'valid',
                 granted_scopes_json = EXCLUDED.granted_scopes_json,
                 updated_at = datetime('now')",
            params![channel_account_id, page_id, encrypted_ciphertext, scopes_json],
        ).map_err(|e| anyhow!("Failed to upsert page token in vault: {}", e))?;
        Ok(())
    }

    pub fn get_valid_page_token(
        conn: &Connection,
        channel_account_id: i64,
        page_id: &str,
    ) -> Result<String> {
        let encrypted_token: Option<String> = conn.query_row(
            "SELECT access_token_encrypted FROM facebook_page_tokens
             WHERE channel_account_id = ?1 AND page_id = ?2 AND token_status = 'valid'",
            params![channel_account_id, page_id],
            |row| row.get(0),
        ).optional().map_err(|e| anyhow!("DB query failed for channel_account_id {} page_id {}: {}", channel_account_id, page_id, e))?;

        let ciphertext = match encrypted_token {
            Some(t) if !t.is_empty() => t,
            _ => return Err(anyhow!("No valid active token found for channel_account_id: {} page_id: {}", channel_account_id, page_id)),
        };

        Self::decrypt_token(&ciphertext)
    }
}
```

#### 2. Schema & Migration Invariants (`src-tauri/src/conversation/migration_v2.rs`)
- Connected-component merger joins attachment records across `remote_url` and `provider_attachment_id`.
- Survivor row is `MIN(id)` with explicit survivor precedence for scalar fields.
- Conflicting JSON metadata keys preserved as `conflict_key_dup_<id>`.
- Non-object / malformed JSON preserved in `_legacy_unparsed_values`.
- Upgrades databases to V4 idempotently.

#### 3. Forward-Only Monotonic Messenger Synchronization (`src-tauri/src/conversation/facebook_sync.rs`)
- Processes events strictly in ascending order.
- Delivery events forward-only update `messages SET status = 'delivered' WHERE conversation_id = ?1 AND provider_message_id = ?2 AND status IN ('sent', 'queued', 'pending')`.
- Read receipts scoped to `direction = 'outbound'` with `provider_timestamp <= watermark`.
- Quarantines invalid reaction timestamps to `channel_sync_dead_letters`.

---

### Task 4: Svelte 5 Unified Inbox & 24-Hour Policy Gate
- 24-Hour Policy Gate evaluates `last_inbound_time`. If > 24 hours, disables regular messaging and enforces Message Tag selection (`CONFIRMED_EVENT_UPDATE`, `POST_PURCHASE_UPDATE`, `ACCOUNT_UPDATE`, `HUMAN_AGENT`).
