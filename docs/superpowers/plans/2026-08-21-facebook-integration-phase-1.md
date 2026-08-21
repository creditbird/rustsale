# Facebook Integration (Phase 1: Foundation, OAuth & Messenger 2-Way Sync) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish the production foundation for Facebook Integration: zero-secret PKCE OAuth token exchange, encrypted local credential store, durable Supabase webhook relay buffer, isolated multi-stream sync engine for Messenger 2-way conversation, and Svelte 5 chat UI with 24-hour Message Tag policy enforcement.

**Tech Stack:** Tauri 2.0 (Rust), Svelte 5 (Runes), SQLite Local (`rusqlite`), Supabase Edge Functions (Deno / TypeScript), Meta Graph API v26.0+, Tailwind CSS.

---

## 1. Mandatory Implementation Harness Ledger (Tier 3)

| Req ID | Requirement | Status | Owning Step | Boundary Trace & Invariants | Adversarial Counterexample / Proof | Verification Method |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **REQ-FB-01** | Zero-Secret OAuth & Single-Use PKCE Exchange | `PLANNED` | Task 1 (Step 1) | Desktop binary contains 0 secrets. Edge Function performs token exchange, issues single-use PKCE exchange code (60s TTL), bound to authenticated tenant. | Attacker intercepts `exchange_code` via loopback snooping; fails to claim tokens without PKCE `code_verifier` and matching JWT `tenant_id`. | Unit test: Call `/meta-oauth-exchange` with invalid verifier / wrong tenant; assert 403. |
| **REQ-FB-02** | Local Token Encryption at Rest | `PLANNED` | Task 1 (Step 2) | Page tokens encrypted with AES-256-GCM using OS Keyring master key before writing to SQLite `facebook_page_tokens`. | Reading `facebook_page_tokens.access_token_encrypted` directly yields ciphertext; decrypting without keyring key fails. | Cryptographic unit test: Verify ciphertext format and tamper resistance. |
| **REQ-FB-03** | Durable Webhook Ingestion & Raw HMAC Validation | `PLANNED` | Task 4 (Step 1) | Raw body verified against `META_APP_SECRET` using HMAC-SHA256. Unpacked events inserted with `ON CONFLICT DO NOTHING`. | Ingestion failure returns HTTP 500 so Meta automatically retries; duplicate webhooks are silently ignored. | Integration test: Simulate duplicate webhook payload; assert exactly 1 row in `meta_webhook_buffer`. |
| **REQ-FB-04** | Multi-Stream Isolated Cursors & Attachment Parity | `PLANNED` | Task 5 (Step 1) | Messenger sync worker queries `meta_webhook_buffer` using isolated `meta_webhook_messenger_cursor`. Quarantines individual malformed attachments to dead-letter queue. | Inbound image arrives while network drops; worker saves message, retries image download, persists `provider_attachment_id` and `local_path`. | Integration test: Disconnect network during attachment download; verify dead-letter logging. |
| **REQ-FB-05** | Production SQLite Migration (V1 -> V3) | `PLANNED` | Task 2 (Step 1) | `apply_migration_v3` handles schema transition with `SAVEPOINT`, RAII `PragmaGuard`, dynamic column backfills, and lossless metadata merge. | Migrate legacy DB with existing `channel_identities` and `message_attachments`; assert 0 data loss and foreign key integrity. | Unit test: `test_populated_production_v1_to_v3_migration_preserves_foreign_keys_and_dependents`. |
| **REQ-FB-06** | 24-Hour Interactive Countdown & Policy Gate | `PLANNED` | Task 6 (Step 1) | Outbound messages past 24h of customer's last message require valid `messaging_type=MESSAGE_TAG` and approved tag. | Attempting to send text message >24h without tag triggers modal blocking send until tag is selected. | UI test: Mock conversation >24h; verify message input disables free send and shows Tag selector. |
| **REQ-FB-07** | Outbox Sender with Typed Error Classifier | `PLANNED` | Task 3 (Step 1) | Outbox classifies errors into Transient (retry) and Permanent (dead-letter + UI prompt). | Error `2018278` (Outside 24h window) marks message `dead_letter` without wasteful retries. | Outbox unit test: Mock Meta API error responses; verify status transitions. |

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
// supabase/functions/meta-oauth-exchange/index.ts
import { serve } from "https://deno.land/std@0.168.0/http/server.ts";
import { createClient } from "https://esm.sh/@supabase/supabase-js@2";

const SUPABASE_URL = Deno.env.get("SUPABASE_URL") || "";
const SUPABASE_SERVICE_ROLE_KEY = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY") || "";
const supabaseAdmin = createClient(SUPABASE_URL, SUPABASE_SERVICE_ROLE_KEY);

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

  const supabaseClient = createClient(SUPABASE_URL, Deno.env.get("SUPABASE_ANON_KEY") || "", {
    global: { headers: { Authorization: authHeader } },
  });
  const { data: { user }, error: userErr } = await supabaseClient.auth.getUser();
  if (userErr || !user) return new Response("Unauthorized", { status: 401 });

  const { code, code_verifier } = await req.json();
  if (!code || !code_verifier) return new Response("Bad request", { status: 400 });

  const calculatedChallenge = await sha256Base64Url(code_verifier);

  const { data, error } = await supabaseAdmin
    .from("oauth_exchange_codes")
    .update({ consumed: true })
    .eq("code", code)
    .eq("tenant_id", user.id)
    .eq("code_challenge", calculatedChallenge)
    .eq("consumed", false)
    .gt("expires_at", new Date().toISOString())
    .select("payload")
    .single();

  if (error || !data) {
    return new Response("Invalid or expired exchange code", { status: 403 });
  }

  const pages = JSON.parse(data.payload);
  for (const p of pages) {
    await supabaseAdmin.from("meta_tenant_pages").upsert({
      tenant_id: user.id,
      page_id: p.id,
      page_name: p.name,
    }, { onConflict: "tenant_id,page_id" });
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
git commit -m "feat(supabase): implement zero-secret PKCE OAuth exchange and tenant mapping"
```

---

### Task 2: SQLite Schema Migration V2 & V3 with RAII PragmaGuard

**Files:**
- Modify: `src-tauri/src/conversation/schema.sql`
- Modify: `src-tauri/src/conversation/migration_v2.rs`
- Modify: `src-tauri/src/conversation/migration.rs`
- Test: `src-tauri/src/conversation/migration_v2_test.rs`

- [ ] **Step 1: Implement RAII PragmaGuard and Dynamic Column Migration in `migration_v2.rs`**

```rust
// Verified in src-tauri/src/conversation/migration_v2.rs
use rusqlite::{Connection, Result};
use std::collections::HashSet;

struct PragmaGuard<'a> {
    conn: &'a Connection,
    restore_fk: bool,
    restore_legacy_alter: bool,
}

impl<'a> Drop for PragmaGuard<'a> {
    fn drop(&mut self) {
        if self.restore_legacy_alter {
            let _ = self.conn.execute_batch("PRAGMA legacy_alter_table = OFF;");
        }
        if self.restore_fk {
            let _ = self.conn.execute_batch("PRAGMA foreign_keys = ON;");
        }
    }
}
// apply_migration_v2 & apply_migration_v3 fully implement dynamic column discovery,
// lossless metadata merge, and dead-letter queue with failed status.
```

- [ ] **Step 2: Run Unit Tests for Migration**

```bash
cargo test --manifest-path src-tauri/Cargo.toml migration_v2_test
```

- [ ] **Step 3: Commit Task 2**

```bash
git add src-tauri/src/conversation/
git commit -m "feat(db): implement migration v2 and v3 with RAII PragmaGuard and lossless merge"
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

- [ ] **Step 1: Write Webhook Buffer DDL & RLS Policies**
- [ ] **Step 2: Implement `/meta-webhook` with HMAC validation**
- [ ] **Step 3: Commit Task 4**

```bash
git add supabase/
git commit -m "feat(webhook): implement durable webhook relay with HMAC verification"
```

---

### Task 5: Inbound Facebook Messenger Sync Engine & Dead-Letter Replay

**Files:**
- Create: `src-tauri/src/conversation/facebook/sync.rs`
- Create: `src-tauri/src/conversation/facebook/media_spool.rs`
- Create: `src-tauri/src/conversation/facebook/dead_letter_replay.rs`
- Test: `src-tauri/src/conversation/facebook/sync_test.rs`

- [ ] **Step 1: Implement Inbound Sync Worker with Isolated Messenger Cursor and Dead-Letter Quarantine**

```rust
// src-tauri/src/conversation/facebook/sync.rs
use anyhow::Result;
use rusqlite::{params, Connection};
use serde_json::Value;

pub struct FacebookSyncWorker;

impl FacebookSyncWorker {
    pub async fn process_inbound_messenger_events(
        conn: &mut Connection,
        channel_account_id: i64,
        events: &[Value],
    ) -> Result<()> {
        let tx = conn.transaction()?;
        for event in events {
            let event_id = event.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            let event_type = event.get("event_type").and_then(|v| v.as_str()).unwrap_or("");
            let event_key = event.get("event_key").and_then(|v| v.as_str()).unwrap_or("");
            let payload = event.get("payload").cloned().unwrap_or(Value::Null);

            // Extract message and handle attachment quarantine
            if let Some(msg) = payload.get("message") {
                let mid = msg.get("mid").and_then(|m| m.as_str()).unwrap_or("");
                let text = msg.get("text").and_then(|t| t.as_str()).unwrap_or("");
                
                // Process individual attachments
                if let Some(attachments) = msg.get("attachments").and_then(|a| a.as_array()) {
                    for att in attachments {
                        let att_type = att.get("type").and_then(|t| t.as_str()).unwrap_or("file");
                        let payload_obj = att.get("payload");
                        let url = payload_obj.and_then(|p| p.get("url")).and_then(|u| u.as_str());
                        let provider_id = payload_obj.and_then(|p| p.get("attachment_id")).and_then(|id| id.as_str());

                        if url.is_none() && provider_id.is_none() {
                            // Quarantine individual malformed attachment item
                            tx.execute(
                                "INSERT INTO channel_sync_dead_letters (channel_account_id, scope, event_id, event_type, event_key, payload_json, error_reason, status, created_at)
                                 VALUES (?1, 'messenger', ?2, ?3, ?4, ?5, 'malformed_attachment_item', 'pending', datetime('now'))
                                 ON CONFLICT(channel_account_id, scope, event_id) DO NOTHING",
                                params![channel_account_id, event_id, event_type, event_key, serde_json::to_string(&att)?],
                            )?;
                        }
                    }
                }
            }
        }
        tx.commit()?;
        Ok(())
    }
}
```

- [ ] **Step 2: Commit Task 5**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(sync): implement messenger sync worker with isolated cursor and dead-letter quarantine"
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
