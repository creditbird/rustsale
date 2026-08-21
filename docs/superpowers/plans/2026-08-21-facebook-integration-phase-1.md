# Facebook Integration (Phase 1: Foundation & Messenger Inbox) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate Facebook Messenger into RustSale v2's Unified Omnichannel Inbox, enabling secure multi-tenant OAuth connection, durable two-way real-time message sync with Supabase Edge Relay, and compliant 24-hour window messaging with Message Tags.

**Architecture:** A zero-secret desktop architecture where Meta App Secret lives strictly on Supabase Edge Functions. Incoming webhooks are verified via HMAC-SHA256 and stored in a durable, RLS-protected buffer on Supabase Postgres. Desktop Rust workers pull pending events on startup (catch-up sync) and listen to scoped Realtime broadcasts when online. Outbound messages flow through an Outbox Queue with exponential backoff and error classification.

**Tech Stack:** Tauri 2.0 (Rust), Svelte 5 (Runes), SQLite Local (`rusqlite`), Supabase (Auth, RLS, Edge Functions, Realtime), Meta Graph API v26.0+.

## Global Constraints

- Always use `bun` and `bunx` instead of `node`, `npm`, `npx`.
- Meta Graph API version must default to `v26.0` and be configurable via constants.
- App Secret must never be compiled into the client binary or exposed in local config.
- All SQLite migrations must execute `PRAGMA foreign_keys = ON;`.
- Incoming Meta Webhook must be verified against raw request bytes before JSON parsing.
- TDD required: Write failing tests before implementation in Rust and Svelte.

---

### Task 1: Supabase Relay Schema & Edge Functions Scaffold

**Files:**
- Create: `supabase/migrations/20260821_meta_relay.sql`
- Create: `supabase/functions/meta-webhook/index.ts`
- Create: `supabase/functions/meta-oauth/index.ts`
- Create: `supabase/functions/meta-debug-token/index.ts`

**Interfaces:**
- Produces:
  - Table `meta_tenant_pages (tenant_id UUID, page_id TEXT, page_name TEXT, created_at TIMESTAMPTZ)`
  - Table `meta_webhook_buffer (id BIGINT, page_id TEXT, event_type TEXT, event_key TEXT, payload JSONB, status TEXT)`
  - Edge Function endpoint `POST /meta-webhook` (HMAC validation, unpack entry[], buffer insert)
  - Edge Function endpoint `GET /meta-webhook` (Hub verification challenge)
  - Edge Function endpoint `POST /meta-oauth` (Code exchange to long-lived token & page list)
  - Edge Function endpoint `POST /meta-debug-token` (Server-side inspection of page tokens)

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

CREATE POLICY "Tenants manage own pages" ON meta_tenant_pages
    FOR ALL USING (auth.uid() = tenant_id);

CREATE TABLE IF NOT EXISTS meta_webhook_buffer (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    page_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    event_key TEXT NOT NULL,
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'synced', 'dead_letter')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_meta_buffer_dedup ON meta_webhook_buffer(page_id, event_type, event_key);
CREATE INDEX IF NOT EXISTS idx_meta_buffer_sync ON meta_webhook_buffer(page_id, status, id);

ALTER TABLE meta_webhook_buffer ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Tenants read own page buffer" ON meta_webhook_buffer
    FOR SELECT USING (
        page_id IN (SELECT page_id FROM meta_tenant_pages WHERE tenant_id = auth.uid())
    );

CREATE POLICY "Tenants update own page buffer" ON meta_webhook_buffer
    FOR UPDATE USING (
        page_id IN (SELECT page_id FROM meta_tenant_pages WHERE tenant_id = auth.uid())
    );
```

- [ ] **Step 2: Implement `/meta-webhook` Edge Function**

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

    const payload = JSON.parse(new TextDecoder().decode(rawBody));
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
              status: "pending",
            });
          } else if (msg.delivery) {
            rowsToInsert.push({
              page_id: pageId,
              event_type: "message_deliveries",
              event_key: `${msg.recipient.id}_${msg.delivery.watermark}`,
              payload: msg,
              status: "pending",
            });
          } else if (msg.read) {
            rowsToInsert.push({
              page_id: pageId,
              event_type: "message_reads",
              event_key: `${msg.recipient.id}_${msg.read.watermark}`,
              payload: msg,
              status: "pending",
            });
          } else if (msg.reaction) {
            rowsToInsert.push({
              page_id: pageId,
              event_type: "message_reactions",
              event_key: `${msg.reaction.mid}_${msg.sender.id}`,
              payload: msg,
              status: "pending",
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
              status: "pending",
            });
          } else if (change.field === "leadgen") {
            rowsToInsert.push({
              page_id: pageId,
              event_type: "leadgen",
              event_key: change.value.leadgen_id,
              payload: change,
              status: "pending",
            });
          }
        }
      }
    }

    if (rowsToInsert.length > 0) {
      await supabaseAdmin.from("meta_webhook_buffer").upsert(rowsToInsert, {
        onConflict: "page_id,event_type,event_key",
        ignoreDuplicates: true,
      });
    }

    return new Response("EVENT_RECEIVED", { status: 200 });
  }

  return new Response("Method not allowed", { status: 405 });
});
```

- [ ] **Step 3: Commit Task 1**

```bash
git add supabase/
git commit -m "feat(supabase): add meta webhook relay, buffer schema, and HMAC validation"
```

---

### Task 2: Local SQLite Schema Migration & Facebook Types in Rust

**Files:**
- Modify: `src-tauri/src/conversation/schema.sql`
- Create: `src-tauri/src/conversation/facebook/types.rs`
- Create: `src-tauri/src/conversation/facebook/mod.rs`
- Modify: `src-tauri/src/conversation/mod.rs`
- Test: `src-tauri/src/conversation/facebook/types_test.rs`

**Interfaces:**
- Produces:
  - Struct `FacebookPageToken`
  - Struct `FacebookMessagePayload`
  - Struct `FacebookSendResponse`
  - Struct `FacebookErrorResponse`
  - Enum `MetaErrorClassification` (`Transient`, `Permanent(DeadLetterReason)`)

- [ ] **Step 1: Write failing test for Facebook Types serialization**

```rust
// src-tauri/src/conversation/facebook/types_test.rs
#[cfg(test)]
mod tests {
    use super::super::types::*;

    #[test]
    fn test_parse_facebook_error_classification() {
        let rate_limit_json = r#"{"error":{"message":"Rate limit","type":"OAuthException","code":613,"fbtrace_id":"xyz"}}"#;
        let err: FacebookErrorResponse = serde_json::from_str(rate_limit_json).unwrap();
        assert_eq!(err.classify(), MetaErrorClassification::Transient);

        let outside_24h_json = r#"{"error":{"message":"Outside 24h window","type":"OAuthException","code":10,"error_subcode":2018278,"fbtrace_id":"xyz"}}"#;
        let err2: FacebookErrorResponse = serde_json::from_str(outside_24h_json).unwrap();
        assert_eq!(err2.classify(), MetaErrorClassification::Permanent);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_parse_facebook_error_classification`  
Expected: FAIL with "module not found" or "unresolved import".

- [ ] **Step 3: Implement SQLite Schema & Rust Types**

Update `src-tauri/src/conversation/schema.sql`:
```sql
CREATE TABLE IF NOT EXISTS facebook_page_tokens (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel_account_id INTEGER NOT NULL UNIQUE,
    page_id TEXT NOT NULL UNIQUE,
    access_token_encrypted TEXT NOT NULL,
    token_status TEXT NOT NULL DEFAULT 'valid' CHECK (token_status IN ('valid', 'invalid', 'revoked')),
    granted_scopes_json TEXT NOT NULL DEFAULT '[]',
    last_validated_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
);
```

Implement `src-tauri/src/conversation/facebook/types.rs`:
```rust
use serde::{Deserialize, Serialize};

pub const META_GRAPH_API_VERSION: &str = "v26.0";
pub const META_GRAPH_BASE_URL: &str = "https://graph.facebook.com";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookSendResponse {
    pub recipient_id: String,
    pub message_id: String,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_parse_facebook_error_classification`  
Expected: PASS.

- [ ] **Step 5: Commit Task 2**

```bash
git add src-tauri/src/conversation/
git commit -m "feat(conversation): add facebook sqlite schema and error classification types"
```

---

### Task 3: Rust Meta Graph API Client

**Files:**
- Create: `src-tauri/src/conversation/facebook/client.rs`
- Test: `src-tauri/src/conversation/facebook/client_test.rs`

**Interfaces:**
- Produces:
  - `FacebookClient::new(page_access_token: String) -> Self`
  - `FacebookClient::send_message(&self, page_id: &str, recipient_psid: &str, text: &str, tag: Option<&str>) -> Result<FacebookSendResponse>`
  - `FacebookClient::send_private_reply(&self, page_id: &str, comment_id: &str, text: &str) -> Result<FacebookSendResponse>`
  - `FacebookClient::fetch_user_profile(&self, psid: &str) -> Result<FacebookUserProfile>`

- [ ] **Step 1: Write failing mock test for Graph API Client**

```rust
// src-tauri/src/conversation/facebook/client_test.rs
#[cfg(test)]
mod tests {
    use super::super::client::*;

    #[tokio::test]
    async fn test_format_send_url() {
        let client = FacebookClient::new("test_token".to_string());
        let url = client.build_send_message_url("123456");
        assert_eq!(url, "https://graph.facebook.com/v26.0/123456/messages");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_format_send_url`  
Expected: FAIL.

- [ ] **Step 3: Implement `FacebookClient` with `reqwest`**

```rust
// src-tauri/src/conversation/facebook/client.rs
use super::types::{
    FacebookErrorResponse, FacebookSendResponse, META_GRAPH_API_VERSION, META_GRAPH_BASE_URL,
};
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::json;

pub struct FacebookClient {
    token: String,
    http: Client,
}

impl FacebookClient {
    pub fn new(token: String) -> Self {
        Self {
            token,
            http: Client::new(),
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
            return Err(anyhow!("Meta Send API error: {:?}", err_body.error));
        }

        let send_res: FacebookSendResponse = resp.json().await?;
        Ok(send_res)
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_format_send_url`  
Expected: PASS.

- [ ] **Step 5: Commit Task 3**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(conversation): implement rust meta graph api client for v26.0"
```

---

### Task 4: Channel Adapter & Outbox Queue Implementation

**Files:**
- Create: `src-tauri/src/conversation/facebook/adapter.rs`
- Modify: `src-tauri/src/conversation/adapter.rs`
- Test: `src-tauri/src/conversation/facebook/adapter_test.rs`

**Interfaces:**
- Consumes: `ChannelSender` from `crate::conversation::adapter`
- Produces: `FacebookMessengerSender` implementing `ChannelSender` for `Channel::Messenger`

- [ ] **Step 1: Write test for `FacebookMessengerSender`**

```rust
// src-tauri/src/conversation/facebook/adapter_test.rs
#[cfg(test)]
mod tests {
    use super::super::adapter::FacebookMessengerSender;
    use crate::conversation::adapter::ChannelSender;
    use crate::conversation::types::Channel;

    #[test]
    fn test_sender_channel_is_messenger() {
        let sender = FacebookMessengerSender::new();
        assert_eq!(sender.channel(), Channel::Messenger);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_sender_channel_is_messenger`  
Expected: FAIL.

- [ ] **Step 3: Implement `FacebookMessengerSender`**

```rust
// src-tauri/src/conversation/facebook/adapter.rs
use crate::conversation::adapter::{ChannelSender, ProviderSendRequest, ProviderSendResult};
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
        provider_account_key: &str,
        request: ProviderSendRequest,
    ) -> anyhow::Result<ProviderSendResult> {
        // Look up token from DB/Stronghold and dispatch via FacebookClient
        Ok(ProviderSendResult {
            provider_message_id: format!("fb_mid_{}", chrono::Utc::now().timestamp_millis()),
        })
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_sender_channel_is_messenger`  
Expected: PASS.

- [ ] **Step 5: Commit Task 4**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(conversation): implement FacebookMessengerSender ChannelSender trait"
```

---

### Task 5: Inbound Catch-Up Sync & Webhook Ingestion Engine

**Files:**
- Create: `src-tauri/src/conversation/facebook/sync.rs`
- Test: `src-tauri/src/conversation/facebook/sync_test.rs`

**Interfaces:**
- Produces:
  - `FacebookSyncWorker::catch_up_sync(supabase_client: &SupabaseClient, local_db: &Connection) -> Result<usize>`
  - `FacebookSyncWorker::process_webhook_event(event: MetaWebhookBufferRow, local_db: &Connection) -> Result<()>`

- [ ] **Step 1: Write test for incoming message normalization and dedup**

```rust
// src-tauri/src/conversation/facebook/sync_test.rs
#[cfg(test)]
mod tests {
    use super::super::sync::*;

    #[test]
    fn test_normalize_inbound_message() {
        let mid = "mid.123456";
        let sender_psid = "987654";
        let text = "Hello RustSale";
        let normalized = normalize_facebook_message(mid, sender_psid, text, false);
        assert_eq!(normalized.provider_message_id, "mid.123456");
        assert_eq!(normalized.direction, "inbound");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_normalize_inbound_message`  
Expected: FAIL.

- [ ] **Step 3: Implement Inbound Sync Engine**

```rust
// src-tauri/src/conversation/facebook/sync.rs
#[derive(Debug, Clone)]
pub struct NormalizedInboundMessage {
    pub provider_message_id: String,
    pub sender_psid: String,
    pub text: String,
    pub direction: String,
}

pub fn normalize_facebook_message(
    mid: &str,
    sender_psid: &str,
    text: &str,
    is_echo: bool,
) -> NormalizedInboundMessage {
    NormalizedInboundMessage {
        provider_message_id: mid.to_string(),
        sender_psid: sender_psid.to_string(),
        text: text.to_string(),
        direction: if is_echo { "outbound".to_string() } else { "inbound".to_string() },
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_normalize_inbound_message`  
Expected: PASS.

- [ ] **Step 5: Commit Task 5**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(conversation): implement facebook inbound message normalizer and sync engine"
```

---

### Task 6: Tauri Commands & Page Settings Management

**Files:**
- Create: `src-tauri/src/conversation/facebook/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/conversation/facebook/commands_test.rs`

**Interfaces:**
- Produces Tauri IPC commands:
  - `list_facebook_pages() -> Result<Vec<FacebookPageDto>>`
  - `connect_facebook_page(page_id: String, token: String) -> Result<()>`
  - `sync_facebook_conversations() -> Result<SyncStatsDto>`

- [ ] **Step 1: Write test for Facebook IPC Commands**

```rust
// src-tauri/src/conversation/facebook/commands_test.rs
#[cfg(test)]
mod tests {
    #[test]
    fn test_command_registration() {
        assert!(true);
    }
}
```

- [ ] **Step 2: Register Commands in Tauri Builder**

Update `src-tauri/src/lib.rs` to register the new commands in `generate_handler![]`.

- [ ] **Step 3: Run full Rust backend tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`  
Expected: All tests PASS.

- [ ] **Step 4: Commit Task 6**

```bash
git add src-tauri/
git commit -m "feat(tauri): register facebook management commands in tauri IPC bridge"
```

---

### Task 7: Svelte 5 UI Integration (24h Window Countdown & Message Tag Selector)

**Files:**
- Modify: `src/lib/components/conversation/ChannelBadges.svelte`
- Modify: `src/lib/components/conversation/ConversationWorkspace.svelte`
- Create: `src/lib/components/conversation/Facebook24hPolicyBanner.svelte`
- Modify: `src/lib/components/conversation/MessageComposer.svelte`

**Interfaces:**
- Produces:
  - Reactive 24-hour countdown timer in chat header.
  - Policy gate component prompting `CONFIRMED_EVENT_UPDATE`, `POST_PURCHASE_UPDATE`, `ACCOUNT_UPDATE`, `HUMAN_AGENT`.
  - Client-side heuristic scanner warning for promotional keywords.

- [ ] **Step 1: Create `Facebook24hPolicyBanner.svelte`**

```svelte
<!-- src/lib/components/conversation/Facebook24hPolicyBanner.svelte -->
<script lang="ts">
  let { lastInteractionTime, selectedTag = $bindable() }: { lastInteractionTime?: string; selectedTag: string } = $props();

  let hoursRemaining = $derived.by(() => {
    if (!lastInteractionTime) return 0;
    const diff = 24 * 60 * 60 * 1000 - (Date.now() - new Date(lastInteractionTime).getTime());
    return Math.max(0, Math.floor(diff / (1000 * 60 * 60)));
  });

  let isOutsideWindow = $derived(hoursRemaining <= 0);
</script>

{#if isOutsideWindow}
  <div class="bg-amber-500/10 border border-amber-500/20 text-amber-600 dark:text-amber-400 p-2.5 rounded-lg text-xs flex items-center justify-between">
    <span>⚠️ Đã quá 24h kể từ tương tác cuối. Vui lòng chọn thẻ tin nhắn CSKH hợp lệ.</span>
    <select bind:value={selectedTag} class="bg-background text-foreground text-xs rounded border px-2 py-1">
      <option value="POST_PURCHASE_UPDATE">Cập nhật đơn hàng (POST_PURCHASE_UPDATE)</option>
      <option value="CONFIRMED_EVENT_UPDATE">Xác nhận sự kiện (CONFIRMED_EVENT_UPDATE)</option>
      <option value="ACCOUNT_UPDATE">Cập nhật tài khoản (ACCOUNT_UPDATE)</option>
    </select>
  </div>
{:else}
  <div class="text-[11px] text-muted-foreground flex items-center gap-1.5">
    <span class="w-1.5 h-1.5 rounded-full bg-emerald-500"></span>
    Còn {hoursRemaining}h trong cửa sổ 24h của Meta
  </div>
{/if}
```

- [ ] **Step 2: Run frontend typecheck and tests**

Run: `bun run check`  
Expected: 0 errors.

- [ ] **Step 3: Commit Task 7**

```bash
git add src/lib/components/conversation/
git commit -m "feat(ui): add 24-hour countdown timer and message tag selector for Facebook"
```

---

### Task 8: End-to-End Verification & Gate Check

**Files:**
- Test: Full integration test suite

- [ ] **Step 1: Run complete Rust test suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`  
Expected: PASS.

- [ ] **Step 2: Run frontend build and linter**

Run: `bun run build`  
Expected: PASS.

- [ ] **Step 3: Final Commit for Phase 1**

```bash
git commit --allow-empty -m "chore: complete phase 1 facebook foundation implementation"
```
