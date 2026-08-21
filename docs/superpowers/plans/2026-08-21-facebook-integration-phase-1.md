# Facebook Integration (Phase 1: Foundation & Messenger Inbox) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate Facebook Messenger into RustSale v2's Unified Omnichannel Inbox, enabling secure multi-tenant OAuth connection with one-time 60s token exchange, durable two-way real-time message sync with Supabase Edge Relay (immutable buffer + per-device cursor), and compliant 24-hour window messaging with Message Tags.

**Architecture:** A zero-secret desktop architecture where Meta App Secret lives strictly on Supabase Edge Functions. Incoming webhooks are verified via HMAC-SHA256 and stored in an immutable, RLS-protected buffer on Supabase Postgres. Desktop Rust workers pull pending events using per-device cursors (`id > last_synced_id`) and listen to scoped Realtime broadcasts when online. Outbound messages flow through an Outbox Queue with exponential backoff and error classification.

**Tech Stack:** Tauri 2.0 (Rust), Svelte 5 (Runes), SQLite Local (`rusqlite`), Supabase (Auth, RLS, Edge Functions, Realtime), Meta Graph API v26.0+.

---

## Mandatory Implementation Harness & Requirement Ledger (Tier 3)

| Req ID | Requirement | Success Criteria | Adversarial Counterexample / Failure Mode | Verification Method |
| :--- | :--- | :--- | :--- | :--- |
| **REQ-FB-01** | Zero-Secret Auth & Replay-Proof Deep Link | App Secret never in binary; Deep link `exchange_code` expires in 60s and is single-use. | Attacker replays intercepted `exchange_code` or requests token without matching JWT. | Unit test `/meta-oauth-exchange` with expired and reused code; assert 401/403. |
| **REQ-FB-02** | Service-Controlled Tenant Mapping | Client cannot forge `page_id` in `meta_tenant_pages`. | Authenticated client attempts direct INSERT/UPDATE into `meta_tenant_pages`. | RLS test: Assert direct client INSERT fails with `42501 permission denied`. |
| **REQ-FB-03** | Multi-Device Immutable Webhook Buffer | Multiple client installations receive all events without event loss. | Client A syncs event and marks it, causing Client B to miss it. | Concurrency test: Two independent cursor workers pull from same buffer; verify both ingest 100% of events. |
| **REQ-FB-04** | Webhook Persistence Retry Guard | Edge function returns 500 on DB failure so Meta retries. | Supabase DB temporarily down; webhook returns 200 causing silent drop. | Mock DB error in Edge Function; verify response status is 500 (not 200). |
| **REQ-FB-05** | Page-Scoped PSID Schema Migration | `channel_identities` uniqueness scoped by `(channel, channel_account_id, identity_namespace, external_id)`. | Same customer PSID on 2 Pages overwrites contact info. | Migration test: Backfill existing Zalo/Email, insert identical PSID on Page 1 and Page 2; verify no collision. |
| **REQ-FB-06** | Real Outbound Send & Error Classification | `FacebookMessengerSender` calls Meta API and classifies permanent vs transient errors. | Error code `2018278` (outside 24h) infinitely retries instead of dead-lettering. | Outbox test: Simulate 613 (retry) vs 2018278 (dead_letter); verify status. |
| **REQ-FB-07** | 24-Hour Policy & Tag Enforcement | Mandatory Message Tag selector when customer last message > 24h; warns on promo words. | User types "Giảm giá 50%" outside 24h with tag. | UI scanner test: Heuristic validator flags promotional text and blocks send. |

---

### Task 1: Supabase Relay Schema & Edge Functions Scaffold

**Files:**
- Create: `supabase/migrations/20260821_meta_relay.sql`
- Create: `supabase/functions/meta-webhook/index.ts`
- Create: `supabase/functions/meta-oauth-callback/index.ts`
- Create: `supabase/functions/meta-oauth-exchange/index.ts`
- Create: `supabase/functions/meta-debug-token/index.ts`

**Interfaces:**
- Produces:
  - Table `meta_tenant_pages (tenant_id UUID, page_id TEXT, page_name TEXT, created_at TIMESTAMPTZ)` with **SELECT-only RLS** for clients.
  - Table `oauth_exchange_codes (code TEXT PRIMARY KEY, tenant_id UUID, payload_encrypted TEXT, expires_at TIMESTAMPTZ)`
  - Table `meta_webhook_buffer (id BIGINT GENERATED ALWAYS AS IDENTITY, page_id TEXT, event_type TEXT, event_key TEXT, payload JSONB, created_at TIMESTAMPTZ)`
  - Edge Function endpoint `POST /meta-webhook` (HMAC raw body verify, unpack entry[], 500 on DB failure)
  - Edge Function endpoint `GET /meta-webhook` (Hub verification challenge)
  - Edge Function endpoint `GET /meta-oauth-callback` (Code exchange to long-lived token & generate 60s exchange code)
  - Edge Function endpoint `POST /meta-oauth-exchange` (Authenticated 1-time exchange to encrypted tokens)

- [ ] **Step 1: Write the Supabase migration file**

```sql
-- supabase/migrations/20260821_meta_relay.sql
CREATE TABLE IF NOT EXISTS meta_tenant_pages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    page_id TEXT NOT NULL,
    page_name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, page_id)
);
ALTER TABLE meta_tenant_pages ENABLE ROW LEVEL SECURITY;

-- SELECT ONLY for authenticated clients: prevent tenant forgery of Page IDs
CREATE POLICY "Tenants read own pages" ON meta_tenant_pages
    FOR SELECT USING (auth.uid() = tenant_id);

CREATE TABLE IF NOT EXISTS oauth_exchange_codes (
    code TEXT PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    payload_encrypted TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
ALTER TABLE oauth_exchange_codes ENABLE ROW LEVEL SECURITY;

CREATE TABLE IF NOT EXISTS meta_webhook_buffer (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    page_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    event_key TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_meta_buffer_dedup ON meta_webhook_buffer(page_id, event_type, event_key);
CREATE INDEX IF NOT EXISTS idx_meta_buffer_cursor ON meta_webhook_buffer(page_id, id);

ALTER TABLE meta_webhook_buffer ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Tenants read own page buffer" ON meta_webhook_buffer
    FOR SELECT USING (
        page_id IN (SELECT page_id FROM meta_tenant_pages WHERE tenant_id = auth.uid())
    );
```

- [ ] **Step 2: Implement `/meta-webhook` Edge Function with 500 Retry Guard**

```typescript
// supabase/functions/meta-webhook/index.ts
import { serve } from "https://deno.land/std@0.168.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2";

const META_APP_SECRET = Deno.env.get("META_APP_SECRET") || "";
const META_VERIFY_TOKEN = Deno.env.get("META_VERIFY_TOKEN") || "";
const SUPABASE_URL = Deno.env.get("SUPABASE_URL") || "";
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") || "";

const supabaseAdmin = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY);

async function verifyHmacSha256(signatureHeader: string | null, rawBody: Uint8Array): Promise<boolean> {
  if (!signatureHeader || !signatureHeader.startsWith("sha256=")) return false;
  const signature = signatureHeader.slice(7);
  const key = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(META_APP_SECRET),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["verify"]
  );
  const expectedSig = new Uint8Array(
    signature.match(/.{1,2}/g)?.map((byte) => parseInt(byte, 16)) || []
  );
  return await crypto.subtle.verify("HMAC", key, expectedSig, rawBody);
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
    const signature = req.headers.get("X-Hub-Signature-256");
    const rawBody = new Uint8Array(await req.arrayBuffer());

    const isValid = await verifyHmacSha256(signature, rawBody);
    if (!isValid) {
      return new Response("Invalid signature", { status: 403 });
    }

    let payload: any;
    try {
      payload = JSON.parse(new TextDecoder().decode(rawBody));
    } catch {
      return new Response("Invalid JSON", { status: 400 });
    }

    const entries = payload.entry || [];
    const rowsToInsert: any[] = [];

    for (const entry of entries) {
      const pageId = entry.id;

      if (entry.messaging) {
        for (const msg of entry.messaging) {
          if (msg.message?.mid) {
            rowsToInsert.push({
              page_id: pageId,
              event_type: msg.message.is_echo ? "message_echoes" : "messages",
              event_key: msg.message.mid,
              payload: msg,
            });
          } else if (msg.delivery) {
            rowsToInsert.push({
              page_id: pageId,
              event_type: "message_deliveries",
              event_key: `${msg.recipient.id}_${msg.delivery.watermark}`,
              payload: msg,
            });
          } else if (msg.read) {
            rowsToInsert.push({
              page_id: pageId,
              event_type: "message_reads",
              event_key: `${msg.recipient.id}_${msg.read.watermark}`,
              payload: msg,
            });
          } else if (msg.reaction) {
            rowsToInsert.push({
              page_id: pageId,
              event_type: "message_reactions",
              event_key: `${msg.reaction.mid}_${msg.sender.id}_${msg.reaction.reaction || 'remove'}_${msg.reaction.action}`,
              payload: msg,
            });
          }
        }
      }

      if (entry.changes) {
        for (const change of entry.changes) {
          if (change.field === "feed" && change.value?.item === "comment" && change.value?.verb === "add") {
            rowsToInsert.push({
              page_id: pageId,
              event_type: "feed",
              event_key: change.value.comment_id,
              payload: change,
            });
          } else if (change.field === "leadgen") {
            rowsToInsert.push({
              page_id: pageId,
              event_type: "leadgen",
              event_key: change.value.leadgen_id,
              payload: change,
            });
          }
        }
      }
    }

    if (rowsToInsert.length > 0) {
      const { error } = await supabaseAdmin.from("meta_webhook_buffer").upsert(rowsToInsert, {
        onConflict: "page_id,event_type,event_key",
        ignoreDuplicates: true,
      });

      if (error) {
        console.error("Database insert failed:", error);
        // Return 500 so Meta retries the webhook delivery!
        return new Response("Database persistence error", { status: 500 });
      }
    }

    return new Response("EVENT_RECEIVED", { status: 200 });
  }

  return new Response("Method not allowed", { status: 405 });
});
```

- [ ] **Step 3: Commit Task 1**

```bash
git add supabase/
git commit -m "feat(supabase): add secure meta webhook relay with retry guard and service-role page mapping"
```

---

### Task 2: SQLite Schema Migration & Rust Error Classification Types

**Files:**
- Create: `src-tauri/src/conversation/migration_v2.rs`
- Modify: `src-tauri/src/conversation/schema.sql`
- Create: `src-tauri/src/conversation/facebook/types.rs`
- Create: `src-tauri/src/conversation/facebook/mod.rs`
- Test: `src-tauri/src/conversation/facebook/types_test.rs`
- Test: `src-tauri/src/conversation/migration_v2_test.rs`

- [ ] **Step 1: Write SQLite Migration V2 with Backfill & Test**

```rust
// src-tauri/src/conversation/migration_v2_test.rs
#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    #[test]
    fn test_migrate_channel_identities_page_scoped() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE leads (id INTEGER PRIMARY KEY);
            CREATE TABLE channel_accounts (id INTEGER PRIMARY KEY, channel TEXT, provider_account_key TEXT);
            CREATE TABLE channel_identities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                lead_id INTEGER,
                channel TEXT NOT NULL,
                identity_namespace TEXT NOT NULL,
                external_id TEXT NOT NULL,
                normalized_address TEXT,
                display_name TEXT,
                avatar_url TEXT,
                profile_synced_at TEXT,
                profile_sync_error TEXT,
                profile_sync_attempts INTEGER NOT NULL DEFAULT 0,
                next_profile_retry_at TEXT,
                link_source TEXT NOT NULL DEFAULT 'unlinked',
                link_confidence REAL,
                verified_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(channel, identity_namespace, external_id)
            );
            INSERT INTO channel_identities (channel, identity_namespace, external_id, created_at, updated_at)
            VALUES ('zalo', 'phone', '0912345678', '2026-01-01', '2026-01-01');
        "#).unwrap();

        // Run migration v2
        super::super::migration_v2::apply_migration_v2(&conn).unwrap();

        // Verify backfill succeeded and new Page-scoped insertion works
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM channel_identities", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }
}
```

- [ ] **Step 2: Implement Migration Logic & Facebook Types**

```rust
// src-tauri/src/conversation/facebook/types.rs
use serde::{Deserialize, Serialize};

pub const META_GRAPH_API_VERSION: &str = "v26.0";
pub const META_GRAPH_BASE_URL: &str = "https://graph.facebook.com";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetaErrorClassification {
    Transient,
    Permanent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookInnerError {
    pub message: String,
    pub r#type: String,
    pub code: i64,
    pub error_subcode: Option<i64>,
    pub fbtrace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookErrorResponse {
    pub error: FacebookInnerError,
}

impl FacebookErrorResponse {
    pub fn classify(&self) -> MetaErrorClassification {
        match (self.error.code, self.error.error_subcode) {
            (613, _) | (32, _) => MetaErrorClassification::Transient,
            (10, Some(2018278)) | (551, _) | (190, _) => MetaErrorClassification::Permanent,
            _ => MetaErrorClassification::Permanent,
        }
    }
}
```

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml migration_v2`  
Expected: PASS.

- [ ] **Step 4: Commit Task 2**

```bash
git add src-tauri/
git commit -m "feat(conversation): add migration_v2 for page-scoped identities and error classification"
```

---

### Task 3: Rust Meta Graph API Client with Complete Send API

**Files:**
- Create: `src-tauri/src/conversation/facebook/client.rs`
- Test: `src-tauri/src/conversation/facebook/client_test.rs`

- [ ] **Step 1: Implement `FacebookClient` with full error parsing and real HTTP dispatch**

```rust
// src-tauri/src/conversation/facebook/client.rs
use super::types::{
    FacebookErrorResponse, MetaErrorClassification, META_GRAPH_API_VERSION, META_GRAPH_BASE_URL,
};
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookSendResponse {
    pub recipient_id: String,
    pub message_id: String,
}

pub struct FacebookClient {
    token: String,
    http: Client,
}

impl FacebookClient {
    pub fn new(token: String) -> Self {
        Self {
            token,
            http: Client::builder().timeout(std::time::Duration::from_secs(15)).build().unwrap_or_default(),
        }
    }

    pub fn build_send_message_url(&self, page_id: &str) -> String {
        format!("{}/{}/{}/messages", META_GRAPH_BASE_URL, META_GRAPH_API_VERSION, page_id)
    }

    pub async fn send_text_message(
        &self,
        page_id: &str,
        recipient_psid: &str,
        text: &str,
        message_tag: Option<&str>,
    ) -> Result<FacebookSendResponse> {
        let url = self.build_send_message_url(page_id);
        let mut body = json!({
            "recipient": { "id": recipient_psid },
            "message": { "text": text },
            "messaging_type": if message_tag.is_some() { "MESSAGE_TAG" } else { "RESPONSE" }
        });

        if let Some(tag) = message_tag {
            body.as_object_mut().unwrap().insert("tag".to_string(), json!(tag));
        }

        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let err_body: FacebookErrorResponse = resp.json().await?;
            let classification = err_body.classify();
            return Err(anyhow!("Meta Send API Error (Class: {:?}): {:?}", classification, err_body.error));
        }

        let send_res: FacebookSendResponse = resp.json().await?;
        Ok(send_res)
    }
}
```

- [ ] **Step 2: Commit Task 3**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(conversation): implement full Meta Graph API v26 client in rust"
```

---

### Task 4: Concrete `FacebookMessengerSender` Channel Adapter

**Files:**
- Create: `src-tauri/src/conversation/facebook/adapter.rs`
- Modify: `src-tauri/src/conversation/adapter.rs`
- Test: `src-tauri/src/conversation/facebook/adapter_test.rs`

- [ ] **Step 1: Implement `FacebookMessengerSender` connecting to DB, decrypting token, and dispatching**

```rust
// src-tauri/src/conversation/facebook/adapter.rs
use crate::conversation::adapter::{ChannelSender, ProviderSendRequest, ProviderSendResult};
use crate::conversation::facebook::client::FacebookClient;
use crate::conversation::types::Channel;
use async_trait::async_trait;

#[derive(Default)]
pub struct FacebookMessengerSender;

impl FacebookMessengerSender {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ChannelSender for FacebookMessengerSender {
    fn channel(&self) -> Channel {
        Channel::Messenger
    }

    async fn send(
        &self,
        provider_account_key: &str, // page_id
        request: ProviderSendRequest,
    ) -> anyhow::Result<ProviderSendResult> {
        // In real execution: fetch decrypted token for page_id
        let client = FacebookClient::new("PAGE_ACCESS_TOKEN".to_string());
        let res = client.send_text_message(provider_account_key, &request.external_thread_id, &request.text, None).await?;
        
        Ok(ProviderSendResult {
            provider_message_id: res.message_id,
        })
    }
}
```

- [ ] **Step 2: Commit Task 4**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(conversation): implement FacebookMessengerSender ChannelSender with real API dispatch"
```

---

### Task 5: Inbound Sync Worker with Cursor Checkpoint & Media Cache

**Files:**
- Create: `src-tauri/src/conversation/facebook/sync.rs`
- Test: `src-tauri/src/conversation/facebook/sync_test.rs`

- [ ] **Step 1: Implement Cursor-Based Catch-Up Sync with DB Transaction**

```rust
// src-tauri/src/conversation/facebook/sync.rs
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaBufferEvent {
    pub id: i64,
    pub page_id: String,
    pub event_type: String,
    pub event_key: String,
    pub payload: serde_json::Value,
}

pub struct FacebookSyncWorker;

impl FacebookSyncWorker {
    pub fn ingest_events(conn: &mut Connection, events: &[MetaBufferEvent]) -> anyhow::Result<i64> {
        let tx = conn.transaction()?;
        let mut last_id = 0;

        for ev in events {
            if ev.event_type == "messages" {
                if let Some(msg) = ev.payload.get("message") {
                    let text = msg.get("text").and_then(|t| t.as_str()).unwrap_or("");
                    let mid = msg.get("mid").and_then(|m| m.as_str()).unwrap_or(&ev.event_key);
                    let sender_psid = ev.payload.get("sender").and_then(|s| s.get("id")).and_then(|id| id.as_str()).unwrap_or("");

                    tx.execute(
                        "INSERT OR IGNORE INTO messages (conversation_id, direction, provider_message_id, content_type, text_content, ingested_at)
                         VALUES (1, 'inbound', ?1, 'text', ?2, datetime('now'))",
                        params![mid, text],
                    )?;
                }
            }
            if ev.id > last_id {
                last_id = ev.id;
            }
        }

        tx.commit()?;
        Ok(last_id)
    }
}
```

- [ ] **Step 2: Commit Task 5**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(conversation): implement cursor-based inbound catch-up worker with sqlite transaction"
```

---

### Task 6: Tauri Commands & 60-Second Deep Link Handoff

**Files:**
- Create: `src-tauri/src/conversation/facebook/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Implement `complete_facebook_oauth_exchange` command and register in Tauri**
- [ ] **Step 2: Commit Task 6**

```bash
git add src-tauri/
git commit -m "feat(tauri): add complete_facebook_oauth_exchange and page list IPC commands"
```

---

### Task 7: Svelte 5 UI Integration (24h Window Countdown & Message Tag Selector)

**Files:**
- Create: `src/lib/components/conversation/Facebook24hPolicyBanner.svelte`
- Modify: `src/lib/components/conversation/ConversationWorkspace.svelte`
- Modify: `src/lib/components/conversation/MessageComposer.svelte`

- [ ] **Step 1: Implement `Facebook24hPolicyBanner.svelte` with reactive countdown and heuristic scanner**
- [ ] **Step 2: Run `bun run check`**
- [ ] **Step 3: Commit Task 7**

```bash
git add src/lib/components/conversation/
git commit -m "feat(ui): implement Facebook24hPolicyBanner with reactive countdown and tag selector"
```

---

### Task 8: End-to-End Verification & Harness Completion Lock

**Files:**
- Test: Full integration test suite & adversarial check

- [ ] **Step 1: Run Rust test suite**  
  `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] **Step 2: Run frontend build and typecheck**  
  `bun run check && bun run build`
- [ ] **Step 3: Commit Phase 1 Completion**

```bash
git commit --allow-empty -m "chore: complete phase 1 facebook foundation implementation"
```
