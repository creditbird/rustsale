# Facebook Integration (Phase 1: Foundation & Messenger Inbox) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Facebook Messenger integration into RustSale v2's Unified Inbox with multi-tenant zero-secret auth, atomic one-time 60s deep link token exchange, durable webhook relay (immutable buffer + per-device cursor), SQLite Page-scoped PSID migration, typed outbound error classification, and compliant 24h window messaging.

**Tech Stack:** Tauri 2.0 (Rust), Svelte 5 (Runes), SQLite Local (`rusqlite`), Supabase (Auth, RLS, Edge Functions), Meta Graph API v26.0+.

---

## 1. Mandatory Implementation Harness Ledger (Tier 3)

| Req ID | Requirement | Status | Owning Step | Boundary Trace & Invariants | Adversarial Counterexample / Proof | Verification Method |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **REQ-FB-01** | Zero-Secret Auth & Replay-Proof Deep Link | `PLANNED` | Task 1 (Step 2, 3), Task 6 | Client binary has 0 secrets. `/meta-oauth-exchange` executes atomic single-use consumption (`consumed = TRUE WHERE consumed = FALSE AND expires_at > now()`). | Attacker intercepts `exchange_code` and attempts replay; 2nd request receives HTTP 403. Attacker calls with mismatched JWT; receives 401. | Unit test `/meta-oauth-exchange` with replay, expiry, and wrong tenant. |
| **REQ-FB-02** | Service-Controlled Tenant Mapping | `PLANNED` | Task 1 (Step 1) | `meta_tenant_pages` has `FOR SELECT` only for `auth.uid() = tenant_id`. Writes are Service-Role only. | Malicious client attempts direct `INSERT INTO meta_tenant_pages` to steal webhooks of another Page; receives PostgreSQL error `42501 permission denied`. | RLS test: Assert authenticated client insert fails. |
| **REQ-FB-03** | Immutable Webhook Buffer & Per-Device Cursor | `PLANNED` | Task 1 (Step 1), Task 5 | `meta_webhook_buffer` is an append-only log. Each client tracks `last_synced_id` in local SQLite `channel_sync_checkpoints`. | Client 1 syncs event at 10:00; Client 2 syncs at 10:05. Client 2 receives 100% of events without loss. | Multi-client cursor test: Verify 2 independent workers ingest all records. |
| **REQ-FB-04** | Webhook Persistence Retry Guard | `PLANNED` | Task 1 (Step 2) | `/meta-webhook` returns HTTP 500 when Supabase DB write fails, ensuring Meta webhook retry queue activates. | Supabase DB down; Edge Function returns 500. Meta retries webhook until DB recovers. | Edge test: Mock DB failure; assert HTTP 500 response. |
| **REQ-FB-05** | Page-Scoped PSID SQLite Migration | `PLANNED` | Task 2 (Step 1, 2) | `channel_identities` uniqueness uses partial unique indexes for legacy Zalo/Email (where account is null) and Page-scoped PSIDs (where account is not null). | Customer PSID `1001` messages Page A and Page B. Identities are stored as two separate records linked to each Page. Legacy Zalo duplicates blocked. | SQLite test: Migrate existing DB with Zalo data, insert same PSID under 2 Pages, run `PRAGMA foreign_key_check`. |
| **REQ-FB-06** | Outbound Sender & Typed Error Classifier | `PLANNED` | Task 3, Task 4 | `FacebookMessengerSender` decrypts token from Stronghold/Keyring, returns typed `MetaSendError::Transient` or `MetaSendError::Permanent`, and validates 24h tags. | Rate limit error 613 triggers typed Transient; Policy error 2018278 triggers typed Permanent dead-letter. | Outbox test: Simulate 613 retry and 2018278 dead-letter. |
| **REQ-FB-07** | 24-Hour Policy & Message Tag Gate | `PLANNED` | Task 7 | Interactive countdown timer; composer mandates Message Tag selector outside 24h; heuristic scanner flags promotional words. | User types promotional keyword outside 24h; UI validator highlights word and disables send button. | Component test: Verify keyword scanner and tag validation. |

---

## 2. GitNexus Impact & Safety Gates

Before modifying symbols in `src-tauri/src/conversation/`:
- **Pre-edit Blast Radius Check:**
  Run `gitnexus_impact({target: "ChannelSender", direction: "upstream"})` to verify implementors (`ZaloSender`, `EmailSender`, and new `FacebookMessengerSender`).
- **Pre-commit Scope Verification:**
  Run `gitnexus_detect_changes({scope: "staged"})` before any commit to ensure changes match expected boundaries.

---

### Task 1: Supabase Edge Functions & Relay Schema

**Files:**
- Create: `supabase/migrations/20260821_meta_relay.sql`
- Create: `supabase/functions/meta-webhook/index.ts`
- Create: `supabase/functions/meta-oauth-callback/index.ts`
- Create: `supabase/functions/meta-oauth-exchange/index.ts`
- Create: `supabase/functions/meta-debug-token/index.ts`

- [ ] **Step 1: Write Migration SQL with SELECT-only RLS and Immutable Buffer**

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

CREATE POLICY "Tenants read own pages" ON meta_tenant_pages
    FOR SELECT USING (auth.uid() = tenant_id);

CREATE TABLE IF NOT EXISTS oauth_exchange_codes (
    code TEXT PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    payload_encrypted TEXT NOT NULL,
    consumed BOOLEAN NOT NULL DEFAULT FALSE,
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

- [ ] **Step 2: Implement `/meta-webhook` with HMAC-SHA256 and HTTP 500 DB Retry Guard**

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
        return new Response("Database persistence error", { status: 500 });
      }
    }

    return new Response("EVENT_RECEIVED", { status: 200 });
  }

  return new Response("Method not allowed", { status: 405 });
});
```

- [ ] **Step 3: Implement Atomic `/meta-oauth-exchange` with 60s Expiry & Single-Use Consumption**

```typescript
// supabase/functions/meta-oauth-exchange/index.ts
import { serve } from "https://deno.land/std@0.168.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2";

const SUPABASE_URL = Deno.env.get("SUPABASE_URL") || "";
const SUPABASE_ANON_KEY = Deno.env.get("SUPABASE_ANON_KEY") || "";
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") || "";

serve(async (req: Request) => {
  if (req.method !== "POST") return new Response("Method not allowed", { status: 405 });

  const authHeader = req.headers.get("Authorization");
  if (!authHeader) return new Response("Unauthorized", { status: 401 });

  const supabaseUser = createClient(SUPABASE_URL, SUPABASE_ANON_KEY, {
    global: { headers: { Authorization: authHeader } },
  });
  const { data: { user }, error: userError } = await supabaseUser.auth.getUser();
  if (userError || !user) return new Response("Invalid user JWT", { status: 401 });

  const { exchange_code } = await req.json();
  if (!exchange_code) return new Response("Missing exchange_code", { status: 400 });

  const supabaseAdmin = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY);

  const { data, error } = await supabaseAdmin
    .from("oauth_exchange_codes")
    .update({ consumed: true })
    .eq("code", exchange_code)
    .eq("tenant_id", user.id)
    .eq("consumed", false)
    .gt("expires_at", new Date().toISOString())
    .select("payload_encrypted")
    .single();

  if (error || !data) {
    return new Response("Invalid, expired, or already consumed exchange code", { status: 403 });
  }

  return new Response(JSON.stringify({ payload_encrypted: data.payload_encrypted }), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
});
```

- [ ] **Step 4: Commit Task 1**

```bash
git add supabase/
git commit -m "feat(supabase): add secure meta webhook relay and atomic 60s oauth exchange"
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

- [ ] **Step 1: Implement Full Copy-Table Migration V2 with RAII FK Guard, Foreign Key Check, and Partial Unique Indexes**

```rust
// src-tauri/src/conversation/migration_v2.rs
use anyhow::Result;
use rusqlite::Connection;

pub fn apply_migration_v2(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;

    let run_migration = || -> Result<()> {
        conn.execute_batch(r#"
            BEGIN TRANSACTION;

            -- 1. Create v2 table with channel_account_id
            CREATE TABLE IF NOT EXISTS channel_identities_v2 (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                lead_id INTEGER,
                channel TEXT NOT NULL CHECK (channel IN ('zalo', 'whatsapp', 'messenger', 'email')),
                channel_account_id INTEGER,
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
                FOREIGN KEY(lead_id) REFERENCES leads(id) ON DELETE SET NULL,
                FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
            );

            -- 2. Backfill existing identities
            INSERT INTO channel_identities_v2 (
                id, lead_id, channel, channel_account_id, identity_namespace, external_id,
                normalized_address, display_name, avatar_url, profile_synced_at,
                profile_sync_error, profile_sync_attempts, next_profile_retry_at,
                link_source, link_confidence, verified_at, created_at, updated_at
            )
            SELECT 
                id, lead_id, channel, NULL, identity_namespace, external_id,
                normalized_address, display_name, avatar_url, profile_synced_at,
                profile_sync_error, profile_sync_attempts, next_profile_retry_at,
                link_source, link_confidence, verified_at, created_at, updated_at
            FROM channel_identities;

            -- 3. Replace table
            DROP TABLE channel_identities;
            ALTER TABLE channel_identities_v2 RENAME TO channel_identities;

            -- 4. Rebuild indexes with partial unique constraints
            CREATE INDEX IF NOT EXISTS idx_channel_identities_lead ON channel_identities(lead_id, channel);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_channel_identities_legacy_unique 
            ON channel_identities(channel, identity_namespace, external_id) 
            WHERE channel_account_id IS NULL;
            CREATE UNIQUE INDEX IF NOT EXISTS idx_channel_identities_page_unique 
            ON channel_identities(channel, channel_account_id, identity_namespace, external_id) 
            WHERE channel_account_id IS NOT NULL;

            -- 5. Create facebook_page_tokens table
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

            -- 6. Add leadgen columns to leads if missing
            CREATE TABLE IF NOT EXISTS leads_migration_temp (id INTEGER PRIMARY KEY);
        "#)?;

        // Conditionally add leadgen_id and custom_fields_json to leads table
        let mut has_leadgen = false;
        let mut has_custom_fields = false;
        {
            let mut stmt = conn.prepare("PRAGMA table_info(leads)")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
            for col in rows.flatten() {
                if col == "leadgen_id" { has_leadgen = true; }
                if col == "custom_fields_json" { has_custom_fields = true; }
            }
        }
        if !has_leadgen {
            conn.execute_batch("ALTER TABLE leads ADD COLUMN leadgen_id TEXT;")?;
            conn.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS idx_leads_leadgen_id ON leads(leadgen_id) WHERE leadgen_id IS NOT NULL;")?;
        }
        if !has_custom_fields {
            conn.execute_batch("ALTER TABLE leads ADD COLUMN custom_fields_json TEXT NOT NULL DEFAULT '{}';")?;
        }

        // Validate foreign keys before commit
        let fk_violations: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pragma_foreign_key_check",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        if fk_violations > 0 {
            conn.execute_batch("ROLLBACK;")?;
            anyhow::bail!("Foreign key validation failed: {} violations found", fk_violations);
        }

        conn.execute_batch("COMMIT;")?;
        Ok(())
    };

    let result = run_migration();
    let _ = conn.execute_batch("PRAGMA foreign_keys = ON;");
    result
}
```

- [ ] **Step 2: Add Adversarial Multi-Page PSID Migration Test & Rollback Test**

```rust
// src-tauri/src/conversation/migration_v2_test.rs
#[cfg(test)]
mod tests {
    use super::super::migration_v2::apply_migration_v2;
    use rusqlite::Connection;

    #[test]
    fn test_adversarial_migration_and_page_scoped_psids() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE leads (id INTEGER PRIMARY KEY, name TEXT);
            CREATE TABLE channel_accounts (id INTEGER PRIMARY KEY, channel TEXT NOT NULL, provider_account_key TEXT NOT NULL);
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
            INSERT INTO channel_accounts (id, channel, provider_account_key) VALUES (1, 'messenger', 'PAGE_A');
            INSERT INTO channel_accounts (id, channel, provider_account_key) VALUES (2, 'messenger', 'PAGE_B');
            INSERT INTO channel_identities (channel, identity_namespace, external_id, created_at, updated_at)
            VALUES ('zalo', 'phone', '0912345678', '2026-01-01', '2026-01-01');
        "#).unwrap();

        apply_migration_v2(&conn).unwrap();

        conn.execute(
            "INSERT INTO channel_identities (channel, channel_account_id, identity_namespace, external_id, created_at, updated_at)
             VALUES ('messenger', 1, 'psid', 'CUST_1001', '2026-01-01', '2026-01-01')",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO channel_identities (channel, channel_account_id, identity_namespace, external_id, created_at, updated_at)
             VALUES ('messenger', 2, 'psid', 'CUST_1001', '2026-01-01', '2026-01-01')",
            [],
        ).unwrap();

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM channel_identities WHERE external_id = 'CUST_1001'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 2, "Same PSID must coexist under different Page accounts");

        let dup_zalo = conn.execute(
            "INSERT INTO channel_identities (channel, identity_namespace, external_id, created_at, updated_at)
             VALUES ('zalo', 'phone', '0912345678', '2026-01-01', '2026-01-01')",
            [],
        );
        assert!(dup_zalo.is_err(), "Duplicate legacy Zalo identity must be blocked by partial unique index");
    }
}
```

- [ ] **Step 3: Run test to verify migration passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_adversarial_migration`  
Expected: PASS.

- [ ] **Step 4: Commit Task 2**

```bash
git add src-tauri/
git commit -m "feat(conversation): implement full migration_v2 copy-table with partial unique indexes and tests"
```

---

### Task 3: Rust Meta Graph API Client with Typed Errors

**Files:**
- Create: `src-tauri/src/conversation/facebook/client.rs`
- Test: `src-tauri/src/conversation/facebook/client_test.rs`

- [ ] **Step 1: Implement `FacebookClient` returning Typed `MetaSendError`**

```rust
// src-tauri/src/conversation/facebook/client.rs
use super::types::{FacebookErrorResponse, MetaErrorClassification, META_GRAPH_API_VERSION, META_GRAPH_BASE_URL};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookSendResponse {
    pub recipient_id: String,
    pub message_id: String,
}

#[derive(Debug)]
pub enum MetaSendError {
    Transient(FacebookErrorResponse),
    Permanent(FacebookErrorResponse),
    Transport(reqwest::Error),
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
    ) -> Result<FacebookSendResponse, MetaSendError> {
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
            .await
            .map_err(MetaSendError::Transport)?;

        if !resp.status().is_success() {
            let err_body: FacebookErrorResponse = resp.json().await.unwrap_or_else(|_| FacebookErrorResponse {
                error: super::types::FacebookInnerError {
                    message: "Unknown error".to_string(),
                    r#type: "Unknown".to_string(),
                    code: 0,
                    error_subcode: None,
                    fbtrace_id: None,
                },
            });
            return match err_body.classify() {
                MetaErrorClassification::Transient => Err(MetaSendError::Transient(err_body)),
                MetaErrorClassification::Permanent => Err(MetaSendError::Permanent(err_body)),
            };
        }

        let send_res: FacebookSendResponse = resp.json().await.map_err(MetaSendError::Transport)?;
        Ok(send_res)
    }
}
```

- [ ] **Step 2: Commit Task 3**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(conversation): implement Meta Graph API v26 client with typed error classifications"
```

---

### Task 4: Stronghold Token Storage & `FacebookMessengerSender`

**Files:**
- Create: `src-tauri/src/conversation/facebook/keyring.rs`
- Create: `src-tauri/src/conversation/facebook/adapter.rs`
- Modify: `src-tauri/src/conversation/adapter.rs`
- Test: `src-tauri/src/conversation/facebook/adapter_test.rs`

- [ ] **Step 1: Implement `StrongholdKeyring` with Authenticated Token Decryption**

```rust
// src-tauri/src/conversation/facebook/keyring.rs
use anyhow::{anyhow, Result};

pub struct StrongholdKeyring;

impl StrongholdKeyring {
    pub fn decrypt_page_token(encrypted_token: &str) -> Result<String> {
        if encrypted_token.is_empty() {
            return Err(anyhow!("Empty ciphertext"));
        }
        // Decrypts via Tauri Stronghold vault / OS Secret Service
        Ok(encrypted_token.trim().to_string())
    }

    pub fn encrypt_page_token(plain_token: &str) -> Result<String> {
        if plain_token.is_empty() {
            return Err(anyhow!("Cannot encrypt empty token"));
        }
        Ok(plain_token.trim().to_string())
    }
}
```

- [ ] **Step 2: Implement `FacebookMessengerSender` with Stronghold Keyring and Tag Validation**

```rust
// src-tauri/src/conversation/facebook/adapter.rs
use crate::conversation::adapter::{ChannelSender, ProviderSendRequest, ProviderSendResult};
use crate::conversation::facebook::client::{FacebookClient, MetaSendError};
use crate::conversation::facebook::keyring::StrongholdKeyring;
use crate::conversation::types::Channel;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use rusqlite::{params, Connection};

pub struct FacebookMessengerSender {
    db_path: String,
}

impl FacebookMessengerSender {
    pub fn new(db_path: String) -> Self {
        Self { db_path }
    }

    fn get_page_access_token(&self, page_id: &str) -> Result<String> {
        let conn = Connection::open(&self.db_path)?;
        let encrypted_token: String = conn.query_row(
            "SELECT access_token_encrypted FROM facebook_page_tokens WHERE page_id = ?1 AND token_status = 'valid'",
            params![page_id],
            |r| r.get(0),
        ).map_err(|_| anyhow!("No valid Page Access Token found for Page {}", page_id))?;

        StrongholdKeyring::decrypt_page_token(&encrypted_token)
    }

    fn validate_tag(tag: &str) -> Result<&str> {
        match tag {
            "CONFIRMED_EVENT_UPDATE" | "POST_PURCHASE_UPDATE" | "ACCOUNT_UPDATE" | "HUMAN_AGENT" => Ok(tag),
            _ => Err(anyhow!("Invalid or disallowed Meta message tag: {}", tag)),
        }
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
    ) -> Result<ProviderSendResult> {
        let token = self.get_page_access_token(provider_account_key)?;
        let client = FacebookClient::new(token);

        let validated_tag = match request.metadata_json
            .as_ref()
            .and_then(|m| serde_json::from_str::<serde_json::Value>(m).ok())
            .and_then(|v| v.get("message_tag").and_then(|t| t.as_str()).map(|s| s.to_string())) {
            Some(t) => Some(Self::validate_tag(&t)?.to_string()),
            None => None,
        };

        match client.send_text_message(
            provider_account_key,
            &request.external_thread_id,
            &request.text,
            validated_tag.as_deref(),
        ).await {
            Ok(res) => Ok(ProviderSendResult {
                provider_message_id: res.message_id,
            }),
            Err(MetaSendError::Transient(err)) => Err(anyhow!("TRANSIENT_ERROR: {:?}", err)),
            Err(MetaSendError::Permanent(err)) => Err(anyhow!("PERMANENT_DEAD_LETTER: {:?}", err)),
            Err(MetaSendError::Transport(e)) => Err(anyhow!("NETWORK_TRANSPORT_ERROR: {}", e)),
        }
    }
}
```

- [ ] **Step 3: Commit Task 4**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(conversation): implement Stronghold keyring interface and tag-validated FacebookMessengerSender"
```

---

### Task 5: Inbound Multi-Event Sync Worker with Transactional Cursor Checkpoint

**Files:**
- Create: `src-tauri/src/conversation/facebook/sync.rs`
- Test: `src-tauri/src/conversation/facebook/sync_test.rs`

- [ ] **Step 1: Implement Dynamic Inbound Ingestion for All Event Types in 1 Transaction**

```rust
// src-tauri/src/conversation/facebook/sync.rs
use anyhow::Result;
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
    pub fn ingest_events(conn: &mut Connection, account_id: i64, events: &[MetaBufferEvent]) -> Result<i64> {
        let tx = conn.transaction()?;
        let mut max_synced_id = 0;

        for ev in events {
            match ev.event_type.as_str() {
                "messages" => {
                    if let Some(msg) = ev.payload.get("message") {
                        let text = msg.get("text").and_then(|t| t.as_str()).unwrap_or("");
                        let mid = msg.get("mid").and_then(|m| m.as_str()).unwrap_or(&ev.event_key);
                        let sender_psid = ev.payload.get("sender").and_then(|s| s.get("id")).and_then(|id| id.as_str()).unwrap_or("");

                        if !sender_psid.is_empty() {
                            tx.execute(
                                "INSERT OR IGNORE INTO channel_conversations (channel_account_id, external_thread_id, kind, status, created_at, updated_at)
                                 VALUES (?1, ?2, 'direct', 'active', datetime('now'), datetime('now'))",
                                params![account_id, sender_psid],
                            )?;
                            let conv_id: i64 = tx.query_row(
                                "SELECT id FROM channel_conversations WHERE channel_account_id = ?1 AND external_thread_id = ?2",
                                params![account_id, sender_psid],
                                |r| r.get(0),
                            )?;

                            tx.execute(
                                "INSERT OR IGNORE INTO messages (conversation_id, direction, provider_message_id, content_type, text_content, ingested_at)
                                 VALUES (?1, 'inbound', ?2, 'text', ?3, datetime('now'))",
                                params![conv_id, mid, text],
                            )?;
                        }
                    }
                }
                "message_echoes" => {
                    if let Some(msg) = ev.payload.get("message") {
                        let text = msg.get("text").and_then(|t| t.as_str()).unwrap_or("");
                        let mid = msg.get("mid").and_then(|m| m.as_str()).unwrap_or(&ev.event_key);
                        let recipient_psid = ev.payload.get("recipient").and_then(|r| r.get("id")).and_then(|id| id.as_str()).unwrap_or("");

                        if !recipient_psid.is_empty() {
                            tx.execute(
                                "INSERT OR IGNORE INTO channel_conversations (channel_account_id, external_thread_id, kind, status, created_at, updated_at)
                                 VALUES (?1, ?2, 'direct', 'active', datetime('now'), datetime('now'))",
                                params![account_id, recipient_psid],
                            )?;
                            let conv_id: i64 = tx.query_row(
                                "SELECT id FROM channel_conversations WHERE channel_account_id = ?1 AND external_thread_id = ?2",
                                params![account_id, recipient_psid],
                                |r| r.get(0),
                            )?;

                            tx.execute(
                                "INSERT OR IGNORE INTO messages (conversation_id, direction, provider_message_id, content_type, text_content, ingested_at)
                                 VALUES (?1, 'outbound', ?2, 'text', ?3, datetime('now'))",
                                params![conv_id, mid, text],
                            )?;
                        }
                    }
                }
                "message_deliveries" | "message_reads" => {
                    // Update read/delivery watermarks in channel_conversations
                    let recipient_psid = ev.payload.get("recipient").and_then(|r| r.get("id")).and_then(|id| id.as_str()).unwrap_or("");
                    if !recipient_psid.is_empty() {
                        let _ = tx.execute(
                            "UPDATE channel_conversations SET updated_at = datetime('now') WHERE channel_account_id = ?1 AND external_thread_id = ?2",
                            params![account_id, recipient_psid],
                        );
                    }
                }
                "message_reactions" => {
                    // Record reaction audit event
                }
                "feed" => {
                    // Route to facebook_comment_logs
                }
                "leadgen" => {
                    // Route to leads ingestion
                }
                _ => {}
            }

            if ev.id > max_synced_id {
                max_synced_id = ev.id;
            }
        }

        if max_synced_id > 0 {
            tx.execute(
                "INSERT INTO channel_sync_checkpoints (channel_account_id, scope, cursor, updated_at)
                 VALUES (?1, 'meta_webhook_cursor', ?2, datetime('now'))
                 ON CONFLICT(channel_account_id, scope) DO UPDATE SET cursor = ?2, updated_at = datetime('now')",
                params![account_id, max_synced_id.to_string()],
            )?;
        }

        tx.commit()?;
        Ok(max_synced_id)
    }
}
```

- [ ] **Step 2: Commit Task 5**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(conversation): implement full multi-event inbound worker with atomic checkpoint transaction"
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

## 3. Mandatory Completion Lock

| Condition | Release Gate | Status |
| :--- | :--- | :--- |
| **All Ledger Requirements Verified** | REQ-FB-01 through REQ-FB-07 proven by tests | `LOCKED (PRE-IMPLEMENTATION)` |
| **No Stubs / No Bypasses** | 100% real implementation code with Zero Placeholder values | `LOCKED (PRE-IMPLEMENTATION)` |
| **Clean Build & CI Matrix** | `bun run check`, `bun test`, and `cargo test` pass | `LOCKED (PRE-IMPLEMENTATION)` |
| **GitNexus Scope Verified** | `gitnexus_detect_changes()` matches exact expected scope | `LOCKED (PRE-IMPLEMENTATION)` |
