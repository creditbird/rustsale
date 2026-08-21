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

const META_APP_SECRET = Deno.env.get("META_APP_SECRET");
const META_VERIFY_TOKEN = Deno.env.get("META_VERIFY_TOKEN");
const SUPABASE_URL = Deno.env.get("SUPABASE_URL") || "";
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") || "";

const supabaseAdmin = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY);

async function verifyHmacSha256(signatureHeader: string | null, rawBody: Uint8Array): Promise<boolean> {
  if (!META_APP_SECRET || !signatureHeader || !signatureHeader.startsWith("sha256=")) return false;
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
    if (!META_VERIFY_TOKEN) {
      return new Response("Configuration error: META_VERIFY_TOKEN not set", { status: 500 });
    }
    const mode = url.searchParams.get("hub.mode");
    const token = url.searchParams.get("hub.verify_token");
    const challenge = url.searchParams.get("hub.challenge");
    if (mode === "subscribe" && token === META_VERIFY_TOKEN) {
      return new Response(challenge, { status: 200 });
    }
    return new Response("Forbidden", { status: 403 });
  }

  if (req.method === "POST") {
    if (!META_APP_SECRET) {
      return new Response("Configuration error: META_APP_SECRET not set", { status: 500 });
    }
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
use anyhow::{anyhow, Result};
use rusqlite::Connection;

pub fn apply_migration_v2(conn: &mut Connection) -> Result<()> {
    let current_version: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap_or(0);
    if current_version >= 2 {
        return Ok(());
    }

    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;

    let migration_result = (|| -> Result<()> {
        let tx = conn.transaction()?;

        tx.execute_batch(r#"
            -- 1. Create v2 table for channel_identities with channel_account_id
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

            -- 6. Create channel_sync_dead_letters table
            CREATE TABLE IF NOT EXISTS channel_sync_dead_letters (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_account_id INTEGER NOT NULL,
                scope TEXT NOT NULL,
                event_id INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                event_key TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                error_reason TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE,
                UNIQUE(channel_account_id, scope, event_id)
            );
            CREATE INDEX IF NOT EXISTS idx_dead_letters_account_scope ON channel_sync_dead_letters(channel_account_id, scope);
        "#)?;

        // Conditionally add provider_timestamp to messages table
        let mut has_provider_timestamp = false;
        {
            let mut stmt = tx.prepare("PRAGMA table_info(messages)")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
            for col in rows.flatten() {
                if col == "provider_timestamp" { has_provider_timestamp = true; }
            }
        }
        if !has_provider_timestamp {
            tx.execute_batch("ALTER TABLE messages ADD COLUMN provider_timestamp INTEGER;")?;
        }

        // Conditionally add leadgen columns to leads table
        let mut has_leadgen = false;
        let mut has_custom_fields = false;
        {
            let mut stmt = tx.prepare("PRAGMA table_info(leads)")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
            for col in rows.flatten() {
                if col == "leadgen_id" { has_leadgen = true; }
                if col == "custom_fields_json" { has_custom_fields = true; }
            }
        }
        if !has_leadgen {
            tx.execute_batch("ALTER TABLE leads ADD COLUMN leadgen_id TEXT;")?;
        }
        tx.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS idx_leads_leadgen_id ON leads(leadgen_id) WHERE leadgen_id IS NOT NULL;")?;
        if !has_custom_fields {
            tx.execute_batch("ALTER TABLE leads ADD COLUMN custom_fields_json TEXT NOT NULL DEFAULT '{}';")?;
        }

        // Check FK violations using a scoped statement
        {
            let mut fk_stmt = tx.prepare("PRAGMA foreign_key_check")?;
            let fk_violations: Vec<String> = fk_stmt
                .query_map([], |row| Ok(row.get::<_, String>(0)?))?
                .filter_map(|r| r.ok())
                .collect();

            if !fk_violations.is_empty() {
                return Err(anyhow!("Foreign key violations found: {:?}", fk_violations));
            }
        }

        tx.execute_batch("PRAGMA user_version = 2;")?;
        tx.commit()?;
        Ok(())
    })();

    let pragma_res = conn.execute_batch("PRAGMA foreign_keys = ON;");
    match migration_result {
        Ok(_) => pragma_res.map_err(|e| anyhow!("Failed to restore foreign keys: {}", e)),
        Err(e) => {
            let _ = pragma_res;
            Err(e)
        }
    }
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
        let mut conn = Connection::open_in_memory().unwrap();
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

        apply_migration_v2(&mut conn).unwrap();

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
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use keyring::{Entry, Error as KeyringError};
use rand::RngCore;

pub struct StrongholdKeyring;

impl StrongholdKeyring {
    fn get_or_create_master_key() -> Result<[u8; 32]> {
        use std::sync::Mutex;
        static INIT_LOCK: Mutex<()> = Mutex::new(());
        let _guard = INIT_LOCK.lock().map_err(|_| anyhow!("Keyring mutex poisoned"))?;

        let entry = Entry::new("rustsale", "facebook_token_master_key")
            .map_err(|e| anyhow!("Failed to access OS Keyring service: {}", e))?;

        match entry.get_password() {
            Ok(pwd_b64) => {
                let raw = BASE64.decode(pwd_b64.trim())
                    .map_err(|e| anyhow!("Failed to decode keyring master key: {}", e))?;
                if raw.len() != 32 {
                    return Err(anyhow!("Keyring master key length invalid"));
                }
                let mut key = [0u8; 32];
                key.copy_from_slice(&raw);
                Ok(key)
            }
            Err(KeyringError::NoEntry) => {
                let mut new_key = [0u8; 32];
                rand::thread_rng().fill_bytes(&mut new_key);
                let b64 = BASE64.encode(&new_key);
                entry.set_password(&b64)
                    .map_err(|e| anyhow!("Failed to persist new master key in OS Keyring: {}", e))?;
                Ok(new_key)
            }
            Err(e) => Err(anyhow!("OS Keyring access error (failed closed): {}", e)),
        }
    }

    pub fn decrypt_page_token(ciphertext_b64: &str) -> Result<String> {
        let key_bytes = Self::get_or_create_master_key()?;
        let raw = BASE64.decode(ciphertext_b64.trim())
            .map_err(|e| anyhow!("Base64 decode failure: {}", e))?;
        if raw.len() < 12 {
            return Err(anyhow!("Ciphertext too short for nonce"));
        }
        let nonce = Nonce::from_slice(&raw[0..12]);
        let cipher = Aes256Gcm::new((&key_bytes).into());
        let decrypted = cipher
            .decrypt(nonce, &raw[12..])
            .map_err(|_| anyhow!("Decryption authentication failed: wrong key or corrupted ciphertext"))?;
        String::from_utf8(decrypted).map_err(|e| anyhow!("Invalid UTF-8 token: {}", e))
    }

    pub fn encrypt_page_token(plain_token: &str) -> Result<String> {
        if plain_token.is_empty() {
            return Err(anyhow!("Cannot encrypt empty token"));
        }
        let key_bytes = Self::get_or_create_master_key()?;
        let cipher = Aes256Gcm::new((&key_bytes).into());
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let mut encrypted = cipher
            .encrypt(nonce, plain_token.as_bytes())
            .map_err(|_| anyhow!("Encryption failure"))?;
        let mut out = nonce_bytes.to_vec();
        out.append(&mut encrypted);
        Ok(BASE64.encode(out))
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

        let res = client.send_text_message(
            provider_account_key,
            &request.external_thread_id,
            &request.text,
            validated_tag.as_deref(),
        ).await.map_err(|e| match e {
            MetaSendError::Transient(err) => anyhow!("TRANSIENT_ERROR: {:?}", err),
            MetaSendError::Permanent(err) => anyhow!("PERMANENT_DEAD_LETTER: {:?}", err),
            MetaSendError::Transport(err) => anyhow!("NETWORK_TRANSPORT_ERROR: {}", err),
        })?;

        Ok(ProviderSendResult {
            provider_message_id: res.message_id,
        })
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
use anyhow::{anyhow, Result};
use reqwest::Client;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaBufferEvent {
    pub id: i64,
    pub page_id: String,
    pub event_type: String,
    pub event_key: String,
    pub payload: Value,
}

pub struct FacebookSyncWorker {
    supabase_url: String,
    supabase_anon_key: String,
    token_provider: Arc<dyn Fn() -> String + Send + Sync>,
    http: Client,
}

impl FacebookSyncWorker {
    pub fn new<F>(supabase_url: String, supabase_anon_key: String, token_provider: F) -> Result<Self>
    where
        F: Fn() -> String + Send + Sync + 'static,
    {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(Self {
            supabase_url,
            supabase_anon_key,
            token_provider: Arc::new(token_provider),
            http: client,
        })
    }

    pub async fn fetch_new_events(&self, page_id: &str, last_synced_id: i64) -> Result<Vec<MetaBufferEvent>> {
        let jwt = (self.token_provider)();
        let url = format!(
            "{}/rest/v1/meta_webhook_buffer?page_id=eq.{}&id=gt.{}&event_type=in.(messages,message_echoes,message_deliveries,message_reads,message_reactions)&order=id.asc&limit=100",
            self.supabase_url, page_id, last_synced_id
        );

        let resp = self.http.get(&url)
            .header("apikey", &self.supabase_anon_key)
            .header("Authorization", format!("Bearer {}", jwt))
            .send().await?;

        if resp.status().as_u16() == 401 {
            return Err(anyhow!("Authentication expired: HTTP 401 Unauthorized"));
        }

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to fetch webhook buffer: HTTP {}", resp.status()));
        }

        let rows: Vec<MetaBufferEvent> = resp.json().await?;
        Ok(rows)
    }

    pub fn ingest_events(conn: &mut Connection, account_id: i64, events: &[MetaBufferEvent]) -> Result<i64> {
        let tx = conn.transaction()?;
        let mut max_synced_id = 0i64;

        for ev in events {
            let mut processed = false;
            match ev.event_type.as_str() {
                "messages" => {
                    if let Some(msg) = ev.payload.get("message") {
                        let text_opt = msg.get("text").and_then(|t| t.as_str());
                        let atts_opt = msg.get("attachments").and_then(|a| a.as_array());
                        let mid = msg.get("mid").and_then(|m| m.as_str()).unwrap_or(&ev.event_key);
                        let sender_psid = ev.payload.get("sender").and_then(|s| s.get("id")).and_then(|id| id.as_str()).unwrap_or("");
                        let timestamp_ms: Option<i64> = ev.payload.get("timestamp").and_then(|t| t.as_i64()).filter(|&t| t > 0);

                        let has_text = text_opt.is_some();
                        let has_atts = atts_opt.map_or(false, |a| !a.is_empty());

                        if (has_text || has_atts) && !sender_psid.is_empty() {
                            tx.execute(
                                "INSERT OR IGNORE INTO channel_identities (channel, channel_account_id, identity_namespace, external_id, created_at, updated_at)
                                 VALUES ('messenger', ?1, 'psid', ?2, datetime('now'), datetime('now'))",
                                params![account_id, sender_psid],
                            )?;
                            let identity_id: i64 = tx.query_row(
                                "SELECT id FROM channel_identities WHERE channel_account_id = ?1 AND external_id = ?2",
                                params![account_id, sender_psid],
                                |r| r.get(0),
                            )?;

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

                            let content_type = if has_atts && !has_text {
                                atts_opt.unwrap().first().and_then(|a| a.get("type")).and_then(|t| t.as_str()).unwrap_or("file")
                            } else {
                                "text"
                            };
                            let text_content = text_opt.unwrap_or("");

                            tx.execute(
                                "INSERT INTO messages (conversation_id, sender_identity_id, direction, provider_message_id, content_type, text_content, provider_timestamp, ingested_at)
                                 VALUES (?1, ?2, 'inbound', ?3, ?4, ?5, CASE WHEN ?6 IS NOT NULL THEN strftime('%Y-%m-%dT%H:%M:%f', ?6 / 1000.0, 'unixepoch') ELSE NULL END, datetime('now'))
                                 ON CONFLICT(conversation_id, provider_message_id) WHERE provider_message_id IS NOT NULL DO UPDATE SET
                                   provider_timestamp = COALESCE(messages.provider_timestamp, excluded.provider_timestamp)
                                 WHERE messages.direction = excluded.direction",
                                params![conv_id, identity_id, mid, content_type, text_content, timestamp_ms],
                            )?;

                            let msg_id_opt: Option<i64> = tx.query_row(
                                "SELECT id FROM messages WHERE conversation_id = ?1 AND provider_message_id = ?2 AND direction = 'inbound'",
                                params![conv_id, mid],
                                |r| r.get(0),
                            ).optional()?;

                            if let Some(msg_id) = msg_id_opt {
                                if let Some(atts) = atts_opt {
                                    for att in atts {
                                        let att_type = att.get("type").and_then(|t| t.as_str()).unwrap_or("file");
                                        let att_url = att.get("payload").and_then(|p| p.get("url")).and_then(|u| u.as_str()).unwrap_or("");
                                        let att_title = att.get("title").and_then(|t| t.as_str());
                                        if !att_url.is_empty() {
                                            tx.execute(
                                                "INSERT OR IGNORE INTO message_attachments (message_id, attachment_type, file_name, remote_url, metadata_json, created_at)
                                                 VALUES (?1, ?2, ?3, ?4, '{}', datetime('now'))",
                                                params![msg_id, att_type, att_title, att_url],
                                            )?;
                                        }
                                    }
                                }

                                tx.execute(
                                    "UPDATE channel_conversations SET last_message_at = datetime('now'), last_synced_at = datetime('now'), updated_at = datetime('now') WHERE id = ?1",
                                    params![conv_id],
                                )?;
                                processed = true;
                            }
                        }
                    }
                }
                "message_echoes" => {
                    if let Some(msg) = ev.payload.get("message") {
                        let text_opt = msg.get("text").and_then(|t| t.as_str());
                        let atts_opt = msg.get("attachments").and_then(|a| a.as_array());
                        let mid = msg.get("mid").and_then(|m| m.as_str()).unwrap_or(&ev.event_key);
                        let recipient_psid = ev.payload.get("recipient").and_then(|r| r.get("id")).and_then(|id| id.as_str()).unwrap_or("");
                        let timestamp_ms: Option<i64> = ev.payload.get("timestamp").and_then(|t| t.as_i64()).filter(|&t| t > 0);

                        let has_text = text_opt.is_some();
                        let has_atts = atts_opt.map_or(false, |a| !a.is_empty());

                        if (has_text || has_atts) && !recipient_psid.is_empty() {
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

                            let content_type = if has_atts && !has_text {
                                atts_opt.unwrap().first().and_then(|a| a.get("type")).and_then(|t| t.as_str()).unwrap_or("file")
                            } else {
                                "text"
                            };
                            let text_content = text_opt.unwrap_or("");

                            tx.execute(
                                "INSERT INTO messages (conversation_id, direction, provider_message_id, content_type, text_content, provider_timestamp, ingested_at)
                                 VALUES (?1, 'outbound', ?2, ?3, ?4, CASE WHEN ?5 IS NOT NULL THEN strftime('%Y-%m-%dT%H:%M:%f', ?5 / 1000.0, 'unixepoch') ELSE NULL END, datetime('now'))
                                 ON CONFLICT(conversation_id, provider_message_id) WHERE provider_message_id IS NOT NULL DO UPDATE SET
                                   provider_timestamp = COALESCE(messages.provider_timestamp, excluded.provider_timestamp)
                                 WHERE messages.direction = excluded.direction",
                                params![conv_id, mid, content_type, text_content, timestamp_ms],
                            )?;

                            let msg_id_opt: Option<i64> = tx.query_row(
                                "SELECT id FROM messages WHERE conversation_id = ?1 AND provider_message_id = ?2 AND direction = 'outbound'",
                                params![conv_id, mid],
                                |r| r.get(0),
                            ).optional()?;

                            if let Some(msg_id) = msg_id_opt {
                                if let Some(atts) = atts_opt {
                                    for att in atts {
                                        let att_type = att.get("type").and_then(|t| t.as_str()).unwrap_or("file");
                                        let att_url = att.get("payload").and_then(|p| p.get("url")).and_then(|u| u.as_str()).unwrap_or("");
                                        let att_title = att.get("title").and_then(|t| t.as_str());
                                        if !att_url.is_empty() {
                                            tx.execute(
                                                "INSERT OR IGNORE INTO message_attachments (message_id, attachment_type, file_name, remote_url, metadata_json, created_at)
                                                 VALUES (?1, ?2, ?3, ?4, '{}', datetime('now'))",
                                                params![msg_id, att_type, att_title, att_url],
                                            )?;
                                        }
                                    }
                                }

                                tx.execute(
                                    "UPDATE channel_conversations SET last_message_at = datetime('now'), last_synced_at = datetime('now'), updated_at = datetime('now') WHERE id = ?1",
                                    params![conv_id],
                                )?;
                                processed = true;
                            }
                        }
                    }
                }
                "message_deliveries" | "message_reads" => {
                    let recipient_psid = ev.payload.get("recipient").and_then(|r| r.get("id")).and_then(|id| id.as_str()).unwrap_or("");
                    let watermark = ev.payload.get("delivery").or_else(|| ev.payload.get("read"))
                        .and_then(|d| d.get("watermark")).and_then(|w| w.as_i64()).unwrap_or(0);

                    if !recipient_psid.is_empty() {
                        if let Some(mids) = ev.payload.get("delivery").and_then(|d| d.get("mids")).and_then(|m| m.as_array()) {
                            let mut all_accounted = !mids.is_empty();
                            for mid_val in mids {
                                if let Some(m_str) = mid_val.as_str() {
                                    let affected = tx.execute(
                                        "INSERT OR IGNORE INTO message_receipts (message_id, recipient_external_id, receipt_type, occurred_at)
                                         SELECT m.id, ?2, 'delivered', datetime('now')
                                         FROM messages m
                                         JOIN channel_conversations c ON m.conversation_id = c.id
                                         WHERE m.provider_message_id = ?1 AND c.channel_account_id = ?3 AND c.external_thread_id = ?2 AND m.direction = 'outbound'",
                                        params![m_str, recipient_psid, account_id],
                                    )?;
                                    if affected == 0 {
                                        let exists: Option<i64> = tx.query_row(
                                            "SELECT 1 FROM message_receipts r
                                             JOIN messages m ON r.message_id = m.id
                                             JOIN channel_conversations c ON m.conversation_id = c.id
                                             WHERE m.provider_message_id = ?1 AND c.channel_account_id = ?3 AND c.external_thread_id = ?2 AND r.recipient_external_id = ?2 AND r.receipt_type = 'delivered'",
                                            params![m_str, recipient_psid, account_id],
                                            |r| r.get(0),
                                        ).optional()?;
                                        if exists.is_none() {
                                            all_accounted = false;
                                        }
                                    }
                                } else {
                                    all_accounted = false;
                                }
                            }
                            if all_accounted {
                                processed = true;
                            }
                        } else if watermark > 0 {
                            let r_type = if ev.event_type == "message_deliveries" { "delivered" } else { "read" };
                            // Cumulative watermark resolution across all preceding qualifying outbound messages using fractional-second ISO timestamp
                            tx.execute(
                                "INSERT OR IGNORE INTO message_receipts (message_id, recipient_external_id, receipt_type, occurred_at)
                                 SELECT m.id, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%f', ?4 / 1000.0, 'unixepoch')
                                 FROM messages m
                                 JOIN channel_conversations c ON m.conversation_id = c.id
                                 WHERE c.channel_account_id = ?1 AND c.external_thread_id = ?2 AND m.direction = 'outbound'
                                   AND (m.provider_timestamp <= strftime('%Y-%m-%dT%H:%M:%f', ?4 / 1000.0, 'unixepoch') OR (m.provider_timestamp IS NULL AND m.ingested_at <= datetime(?4 / 1000, 'unixepoch')))",
                                params![account_id, recipient_psid, r_type, watermark],
                            )?;
                            processed = true;
                        }
                    }
                }
                "message_reactions" => {
                    if let Some(r) = ev.payload.get("reaction") {
                        let mid = r.get("mid").and_then(|m| m.as_str()).unwrap_or("");
                        let sender_psid = ev.payload.get("sender").and_then(|s| s.get("id")).and_then(|id| id.as_str()).unwrap_or("");
                        let reaction = r.get("reaction").and_then(|re| re.as_str()).unwrap_or("");
                        let action = r.get("action").and_then(|a| a.as_str()).unwrap_or("");

                        if !mid.is_empty() && !sender_psid.is_empty() {
                            tx.execute(
                                "INSERT OR REPLACE INTO pending_message_reactions (
                                    channel_account_id, external_thread_id, conversation_kind, provider_message_id, actor_external_id, reaction, removed, occurred_at, created_at, updated_at
                                 ) VALUES (?1, ?2, 'direct', ?3, ?2, ?4, ?5, datetime('now'), datetime('now'), datetime('now'))",
                                params![account_id, sender_psid, mid, reaction, if action == "unreact" { 1 } else { 0 }],
                            )?;
                            processed = true;
                        }
                    }
                }
                _ => {
                    // Feed and Leadgen events are handled by dedicated Phase 3 workers
                }
            }

            if !processed {
                // Route unhandled / malformed payload to dedicated durable dead-letter table with full diagnostic metadata
                tx.execute(
                    "INSERT OR IGNORE INTO channel_sync_dead_letters (channel_account_id, scope, event_id, event_type, event_key, payload_json, error_reason, created_at)
                     VALUES (?1, 'messenger', ?2, ?3, ?4, ?5, ?6, datetime('now'))",
                    params![account_id, ev.id, ev.event_type, ev.event_key, ev.payload.to_string(), "unhandled_or_malformed_payload"],
                )?;
                processed = true;
            }

            if processed && ev.id > max_synced_id {
                max_synced_id = ev.id;
            }
        }

        if max_synced_id > 0 {
            tx.execute(
                "INSERT INTO channel_sync_checkpoints (channel_account_id, scope, cursor, last_event_at, updated_at)
                 VALUES (?1, 'messenger', ?2, datetime('now'), datetime('now'))
                 ON CONFLICT(channel_account_id, scope) DO UPDATE SET cursor = ?2, last_event_at = datetime('now'), updated_at = datetime('now')",
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
