# Facebook Integration Design Specification — RustSale v2

**Date:** 2026-08-21  
**Status:** Implementation-Ready Architectural Design Specification (Remediated for GPT-5.6 Sol / Fable 5 Production Standards)  
**Author / Reviewer:** Antigravity Team & Lead Architect  
**Target Platform:** Tauri 2.0 (Rust) + Svelte 5 (Runes) + SQLite Local + Supabase (Auth, RLS, Edge Functions)

---

## 1. Executive Summary & Architecture Overview

The Facebook Integration module empowers RustSale v2 with enterprise-grade Meta capabilities across three pillars:
1. **Unified Omnichannel Messenger Inbox (`/conversation`)**: Seamless 2-way real-time messaging with Facebook Pages, integrating directly into the existing RustSale unified inbox alongside Zalo and Email.
2. **Facebook Post Scheduler & Planner (`/facebook`)**: Dual-mode scheduling (Meta-side 10m–30d & Local-queued >30d) supporting Status, Link, Single Photo, Multi-photo Albums, Resumable Videos, and Reels with live Desktop/Mobile feed previews.
3. **Automations & Growth Engine**: Compliant Private Reply for comments with in-flight heartbeat and fail-closed reconciliation, 24-hour window countdown with Message Tag gating, Lead Ads instant CRM ingestion, proactive Page Health monitoring, and User Data Deletion compliance.

### High-Level Architecture

```mermaid
flowchart TD
    subgraph Meta_Cloud ["Meta Cloud Platform (Graph API v26.0+)"]
        FB_User["Khách hàng Facebook"]
        FB_Send["Send API / Feed API / Video Graph API"]
        FB_Hook["Meta Webhooks (messages, feed, leadgen)"]
    end

    subgraph Supabase_Cloud ["Supabase Cloud Relay & Auth Bridge"]
        EdgeOAuthStart["Edge Function: /meta-oauth-start (PKCE & Session Init)"]
        EdgeOAuth["Edge Function: /meta-oauth-callback (Verify & Exchange)"]
        EdgeExchange["Edge Function: /meta-oauth-exchange (Authenticated 1-Time PKCE Handoff)"]
        EdgeDebug["Edge Function: /meta-debug-token (Inspect Scopes)"]
        EdgeHook["Edge Function: /meta-webhook (HMAC Raw Body Verify & Retry Guard)"]
        EdgeDelete["Edge Function: /meta-data-deletion (Signed Request Verify & Status Endpoint)"]
        TenantMap[("meta_tenant_pages (Service-Role Protected)")]
        BufferDB[("meta_webhook_buffer (Immutable Log + RLS)")]
        AuthSessionsDB[("oauth_authorization_sessions (PKCE Challenges)")]
        ExchangeDB[("oauth_exchange_codes (Atomic 60s Single-Use PKCE)")]
        DeletionDB[("data_deletion_requests & meta_device_deletion_tasks")]
        Realtime["Supabase Realtime (Tenant Scoped Private Channel)"]
    end

    subgraph Desktop_App ["RustSale Desktop App (Local-First Native)"]
        DeepLink["tauri-plugin-deep-link (rustsale://oauth/facebook/callback?code=...&state=...)"]
        Stronghold["Tauri Stronghold / OS Keyring (AES-256-GCM Local Master Key)"]
        RustSyncWorker["Rust Inbound Sync Worker (Multi-Stream Isolated Cursors)"]
        RustCommentWorker["Rust Comment Automation Worker (5-Min Lease Lock + Heartbeat + Reconciliation)"]
        RustPostWorker["Rust Post Scheduler Worker (Restart-Resumable Uploads & JIT Promotion)"]
        RustOutbox["Rust Outbox Sender (Exponential Backoff + Typed Error Classifier)"]
        SQLite[("Local SQLite DB (channel_accounts, messages, posts, leads, tokens)")]
        MediaCache["Local Media Cache (Sanitized Spool / Attachments)"]
        SvelteUI["Svelte 5 Runes UI (/conversation, /facebook, /hub)"]
    end

    %% Auth Flow
    SvelteUI -->|1. Generate PKCE & Init| EdgeOAuthStart
    EdgeOAuthStart -->|Store Session & Challenge| AuthSessionsDB
    EdgeOAuthStart -->|Open Meta Dialog| DeepLink
    EdgeOAuth -->|Store Bound 60s Single-Use Code| ExchangeDB
    DeepLink -->|POST /meta-oauth-exchange with JWT & PKCE Verifier| EdgeExchange
    EdgeExchange -->|Return TLS-Protected Tokens & Erase Payload| Stronghold
    Stronghold -->|Encrypt with OS Keyring Key| SQLite

    %% Webhook Flow
    FB_User -->|Nhắn tin / Comment / Lead| FB_Hook
    FB_Hook -->|POST with X-Hub-Signature-256| EdgeHook
    EdgeHook -->|Unpack entry[] & INSERT| BufferDB
    EdgeHook -.->|500 on DB Error (Meta Retries)| FB_Hook
    BufferDB -->|Broadcast Event| Realtime
    Realtime -.->|Live Event| RustSyncWorker
    RustSyncWorker -->|Cursor-Based Pull WHERE id > last_cursor| SQLite
    RustSyncWorker -->|Tải Media Cục bộ| MediaCache
    SQLite -->|Reactive Timeline| SvelteUI

    %% Outbound & Automations
    SvelteUI -->|Gửi tin / Đăng bài| RustOutbox
    RustOutbox -->|POST /<page_id>/messages /feed| FB_Send
    RustCommentWorker -->|Private Reply with Lock & Heartbeat| FB_Send
    RustPostWorker -->|Resumable Chunked Transfer| FB_Send
```

---

## 2. Security, Authentication & Cryptographic Token Lifecycle

### 2.1 Zero-Secret Desktop Architecture & Complete PKCE Token Handoff
- **Strict Mandate:** Meta `client_secret` and App Access Tokens (`app_id|app_secret`) MUST NEVER reside within the desktop binary.
- **End-to-End Cryptographic Protocol with PKCE & Server-Verifiable Binding:**
  1. **PKCE & Session Initiation:**
     - Desktop client generates a random UUIDv4 `state` and a cryptographically random 32-byte string `code_verifier`.
     - Desktop computes `code_challenge = base64url(sha256(code_verifier))`.
     - Desktop calls authenticated Supabase Edge Function `POST /meta-oauth-start` with user's JWT (`Authorization: Bearer <user_jwt>`) and body `{ state, code_challenge }`.
     - Edge Function inserts `{ state, tenant_id: auth.uid(), code_challenge, expires_at: now() + interval '10 minutes' }` into `oauth_authorization_sessions` under RLS.
  2. **Authorization Code Flow:**
     - Desktop opens system browser to Meta OAuth dialog:
       `https://www.facebook.com/v26.0/dialog/oauth?client_id=<app_id>&redirect_uri=<supabase_url>/functions/v1/meta-oauth-callback&state=<state>&scope=pages_messaging,pages_manage_posts,pages_read_engagement,pages_show_list,leads_retrieval`
  3. **Server-Side Token Exchange & Pre-Binding:**
     - Meta redirects user to `/meta-oauth-callback`. Edge Function:
       - Looks up and atomically deletes `oauth_authorization_sessions WHERE state = $state AND expires_at > now() RETURNING tenant_id, code_challenge`. If not found, returns HTTP 403 (prevents CSRF & replay).
       - Exchanges `code` for short-lived user token via Meta Graph API using server-side `META_APP_SECRET`.
       - Exchanges short-lived token for 60-day **Long-lived User Token**.
       - Queries `GET /me/accounts` to retrieve all Page Access Tokens.
       - Generates a cryptographically random 60-second single-use `exchange_code` (UUIDv4).
       - Persists `{ code: exchange_code, tenant_id: session.tenant_id, code_challenge: session.code_challenge, payload: JSON.stringify(pages), expires_at: now() + interval '60 seconds', consumed: false }` in `oauth_exchange_codes`.
       - Redirects browser to desktop custom protocol: `rustsale://oauth/facebook/callback?code=<exchange_code>&state=<state>`.
  4. **Client-Side CSRF Verification & Authenticated PKCE Exchange:**
     - Desktop catches deep-link callback via `tauri-plugin-deep-link`.
     - Desktop verifies callback `state` matches in-memory `state`.
     - Desktop sends authenticated HTTPS POST request to `/meta-oauth-exchange` with user's Supabase Auth JWT header (`Authorization: Bearer <user_jwt>`) and body `{ code: exchange_code, code_verifier: code_verifier }`.
  5. **Atomic Single-Use Consumption, Tenant Mapping & Payload Erasure:**
     - Edge Function calls Postgres RPC function `complete_meta_oauth_exchange(p_code, p_tenant_id, p_challenge)` in a single transaction:
       - Verifies `auth.uid() == tenant_id` and `code_challenge == base64url(sha256(code_verifier))`.
       - Atomically marks `consumed = true` and erases `payload = ''` to prevent token leakage in backups.
       - Upserts Page ownership into `meta_tenant_pages (tenant_id, page_id, page_name)` under RLS.
       - Returns the Page Tokens over authenticated TLS directly to the desktop client.
       - If any step fails, the transaction rolls back cleanly.
  6. **Local Encryption at Rest:**
     - Desktop client receives Page Tokens over TLS and immediately encrypts `access_token` using AES-256-GCM via `StrongholdKeyring::encrypt_page_token(page_token)` before persisting into SQLite `facebook_page_tokens` table.
     - Master key is generated locally on first launch, stored in OS Keyring (Windows Credential Manager / Linux Secret Service / macOS Keychain), never leaving the device.

### 2.2 Token Inspection & Status Machine
- Scheduled background worker inspects tokens every 24 hours via `/meta-debug-token`.
- Token States:
  - `valid`: Token active and operational.
  - `invalid`: Token expired or scopes insufficient.
  - `revoked`: User password changed, permissions removed, or admin role lost on Page. Emits `TokenRevoked` event to UI.

---

## 3. Database Schema & SQLite Data Architecture

All SQLite database migrations enforce `PRAGMA foreign_keys = ON;`.

### 3.1 SQLite Local Database (Desktop) & Complete Lifecycle Tables

```sql
-- 1. Quản lý Token mã hóa của từng Fanpage
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

-- 2. Chuẩn hóa channel_identities hỗ trợ Page-Scoped PSID và Unique cho Legacy Channels
CREATE TABLE IF NOT EXISTS channel_identities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    lead_id INTEGER,
    channel TEXT NOT NULL CHECK (channel IN ('zalo', 'whatsapp', 'messenger', 'email')),
    channel_account_id INTEGER, -- NULL cho Zalo/Email, NOT NULL cho Facebook Page-scoped PSID
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

CREATE INDEX IF NOT EXISTS idx_channel_identities_lead ON channel_identities(lead_id, channel);
CREATE UNIQUE INDEX IF NOT EXISTS idx_channel_identities_legacy_unique 
ON channel_identities(channel, identity_namespace, external_id) 
WHERE channel_account_id IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_channel_identities_page_unique 
ON channel_identities(channel, channel_account_id, identity_namespace, external_id) 
WHERE channel_account_id IS NOT NULL;

-- 3. Dead-Letter Queue với đầy đủ Lifecycle Columns & Atomic Worker Claim
CREATE TABLE IF NOT EXISTS channel_sync_dead_letters (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel_account_id INTEGER NOT NULL,
    scope TEXT NOT NULL,
    event_id INTEGER NOT NULL,
    item_index INTEGER NOT NULL DEFAULT 0,
    event_type TEXT NOT NULL,
    event_key TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    error_reason TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'retrying', 'resolved', 'ignored', 'failed')),
    attempts INTEGER NOT NULL DEFAULT 0,
    next_retry_at TEXT,
    owner_id TEXT,
    claim_deadline TEXT,
    resolved_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE,
    UNIQUE(channel_account_id, scope, event_id, item_index)
);
CREATE INDEX IF NOT EXISTS idx_dead_letters_account_scope ON channel_sync_dead_letters(channel_account_id, scope);
CREATE INDEX IF NOT EXISTS idx_dead_letters_status ON channel_sync_dead_letters(status, next_retry_at);

-- 4. Session tải lên Resumable Video & Reels (Chống mất phiên khi restart app)
CREATE TABLE IF NOT EXISTS facebook_upload_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    page_id TEXT NOT NULL,
    upload_type TEXT NOT NULL DEFAULT 'video' CHECK (upload_type IN ('video', 'reel')),
    upload_session_id TEXT NOT NULL UNIQUE,
    video_id TEXT NOT NULL,
    file_path TEXT NOT NULL,
    file_hash TEXT NOT NULL,
    file_size_bytes INTEGER NOT NULL,
    start_offset INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'in_progress' CHECK (status IN ('in_progress', 'completed', 'failed')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_fb_upload_active_unique ON facebook_upload_sessions(page_id, file_hash, file_size_bytes) WHERE status = 'in_progress';

-- 5. Bảng quản lý Lên lịch đăng bài (Post Scheduler)
CREATE TABLE IF NOT EXISTS facebook_scheduled_posts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    draft_group_id TEXT NOT NULL,
    channel_account_id INTEGER NOT NULL,
    page_id TEXT NOT NULL,
    post_type TEXT NOT NULL CHECK (post_type IN ('status', 'link', 'photo', 'multi_photo', 'video', 'reel')),
    message TEXT NOT NULL,
    link_url TEXT,
    media_paths_json TEXT NOT NULL DEFAULT '[]',
    scheduling_mode TEXT NOT NULL DEFAULT 'meta' CHECK (scheduling_mode IN ('meta', 'local_queued')),
    scheduled_publish_time TEXT,
    remote_post_id TEXT,
    upload_session_id TEXT,
    idempotency_key TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'queued', 'publishing', 'scheduled_on_meta', 'published', 'failed', 'delivery_unknown', 'overdue_prompt')),
    retry_count INTEGER NOT NULL DEFAULT 0,
    owner_id TEXT,
    claim_deadline TEXT,
    error_code TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_fb_scheduled_posts_status ON facebook_scheduled_posts(status, scheduled_publish_time);
CREATE INDEX IF NOT EXISTS idx_fb_scheduled_posts_group ON facebook_scheduled_posts(draft_group_id);

-- 6. Bảng cấu hình Tự động phản hồi bình luận (Comment Automation)
CREATE TABLE IF NOT EXISTS facebook_comment_automations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel_account_id INTEGER NOT NULL,
    page_id TEXT NOT NULL,
    rule_name TEXT NOT NULL,
    post_ids_json TEXT NOT NULL DEFAULT '[]',
    keywords_json TEXT NOT NULL DEFAULT '[]',
    match_mode TEXT NOT NULL DEFAULT 'contains' CHECK (match_mode IN ('contains', 'exact', 'regex', 'all')),
    auto_like INTEGER NOT NULL DEFAULT 1 CHECK (auto_like IN (0, 1)),
    public_reply_template TEXT,
    private_reply_template TEXT,
    is_enabled INTEGER NOT NULL DEFAULT 1 CHECK (is_enabled IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
);

-- 7. Bảng nhật ký phản hồi comment (Lease Lock, Ambiguous Delivery & Audit Log)
CREATE TABLE IF NOT EXISTS facebook_comment_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    comment_id TEXT NOT NULL UNIQUE,
    page_id TEXT NOT NULL,
    post_id TEXT,
    sender_psid TEXT NOT NULL,
    sender_name TEXT,
    comment_text TEXT,
    automation_id INTEGER,
    like_status TEXT NOT NULL DEFAULT 'none' CHECK (like_status IN ('none', 'processing', 'success', 'failed')),
    public_reply_status TEXT NOT NULL DEFAULT 'none' CHECK (public_reply_status IN ('none', 'processing', 'success', 'failed')),
    private_reply_status TEXT NOT NULL DEFAULT 'none' CHECK (private_reply_status IN ('none', 'processing', 'success', 'failed', 'skipped', 'delivery_unknown')),
    owner_id TEXT,
    claim_deadline TEXT,
    skip_reason TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY(automation_id) REFERENCES facebook_comment_automations(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_fb_comment_logs_page_date ON facebook_comment_logs(page_id, created_at);
```

### 3.2 Supabase Cloud Relay Schema & Multi-Tenant RLS

```sql
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
CREATE INDEX IF NOT EXISTS idx_meta_buffer_type ON meta_webhook_buffer(page_id, event_type, id);

ALTER TABLE meta_webhook_buffer ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Tenants read own page buffer" ON meta_webhook_buffer
    FOR SELECT USING (
        page_id IN (SELECT page_id FROM meta_tenant_pages WHERE tenant_id = auth.uid())
    );

CREATE TABLE IF NOT EXISTS data_deletion_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id TEXT NOT NULL,
    confirmation_code TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ
);
ALTER TABLE data_deletion_requests ENABLE ROW LEVEL SECURITY;

CREATE TABLE IF NOT EXISTS meta_device_deletion_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL REFERENCES data_deletion_requests(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'acknowledged', 'completed')),
    acknowledged_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
ALTER TABLE meta_device_deletion_tasks ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Tenants sync own deletion tasks" ON meta_device_deletion_tasks
    FOR ALL USING (auth.uid() = tenant_id);
```

---

## 4. Multi-Stream Webhook Ingestion & Isolated Cursor Protocol

### 4.1 Independent Consumer Cursor Scopes
To prevent any event loss or skipping across different consumer workers (Messenger sync, Comment automations, and Lead Ads ingestion), the system utilizes **independent cursor scopes** per event stream in `channel_sync_checkpoints`:

1. `meta_webhook_messenger_cursor`:
   - Scopes: `messages`, `message_echoes`, `message_deliveries`, `message_reads`, `message_reactions`.
   - Query: `SELECT * FROM meta_webhook_buffer WHERE page_id = ? AND id > ? AND event_type IN ('messages', 'message_echoes', 'message_deliveries', 'message_reads', 'message_reactions') ORDER BY id ASC LIMIT 100`.
2. `meta_webhook_feed_cursor`:
   - Scopes: `feed` (comments, shares, post updates), `mention`.
   - Query: `SELECT * FROM meta_webhook_buffer WHERE page_id = ? AND id > ? AND event_type IN ('feed', 'mention') ORDER BY id ASC LIMIT 100`.
3. `meta_webhook_leadgen_cursor`:
   - Scopes: `leadgen`.
   - Query: `SELECT * FROM meta_webhook_buffer WHERE page_id = ? AND id > ? AND event_type = 'leadgen' ORDER BY id ASC LIMIT 100`.

### 4.2 Webhook Ingestion with HMAC-SHA256 & Retry Guard
- Edge Function `/meta-webhook` verifies `X-Hub-Signature-256` raw body hash.
- Unpacks all entries and performs atomic batch insert with `ON CONFLICT (page_id, event_type, event_key) DO NOTHING`.
- If database insert fails, returns HTTP 500 so Meta automatically retries.
- On success: Returns HTTP 200 OK and broadcasts to tenant private channels.

### 4.3 Dead-Letter Recovery & Replay Worker
- Failed payload ingestion logs to `channel_sync_dead_letters` with `item_index`.
- Background replay worker atomically claims work:
  ```sql
  UPDATE channel_sync_dead_letters
  SET status = CASE WHEN attempts + 1 >= 5 THEN 'failed' ELSE 'retrying' END,
      owner_id = ?1,
      claim_deadline = datetime('now', '+5 minutes'),
      attempts = attempts + 1
  WHERE id = ?2 
    AND status IN ('pending', 'retrying')
    AND (claim_deadline IS NULL OR claim_deadline < datetime('now'))
    AND (next_retry_at IS NULL OR next_retry_at <= datetime('now'))
    AND attempts < 5;
  ```
- Replays payload with exponential backoff (`next_retry_at = datetime('now', '+' || (attempts * attempts * 30) || ' seconds')`).
- Upon success, marks `status = 'resolved'`, `resolved_at = datetime('now')`, requiring matching `owner_id`.
- Expired lease sweeper transitions timed-out claims back to `pending` (or `failed` if `attempts >= 5`).

---

## 5. Outbound Messaging & 24-Hour Policy Compliance

### 5.1 Outbox Queue & Typed Error Classification

| Error Category | Subcodes / Error Types | Action |
| :--- | :--- | :--- |
| **Transient Errors** | `613` (Rate limit), `32` (Page throttle), Network Timeout | Exponential Backoff Retry (1s, 2s, 4s, 8s... max 5 retries). |
| **Permanent Errors** | `2018278` (Outside 24h window without tag), `551` (User blocked Page), `190` (Token invalid/revoked) | Mark `status = 'dead_letter'`, halt retries, render actionable prompt in UI. |

### 5.2 24-Hour Window & Message Tag Gate
- **Interactive Countdown**: UI displays live countdown from customer's latest inbound timestamp.
- **Tag Enforcement**: When countdown expires, composer forces selection of approved Message Tag:
  - `CONFIRMED_EVENT_UPDATE`
  - `POST_PURCHASE_UPDATE`
  - `ACCOUNT_UPDATE`
  - `HUMAN_AGENT`

---

## 6. Post Scheduler, Resumable Media & Fan-Out Engine

### 6.1 Multi-Page Fan-Out & Durability
- User scheduling a post across $N$ Pages generates deterministic client draft IDs:
  `client_draft_id = UUIDv5(namespace, format!("{}:{}:{}", user_id, base_post_id, page_id))`
- Inserts $N$ atomic rows in SQLite inside a single transaction.

### 6.2 Resumable Video & Reels Upload State Machine
- Table `facebook_upload_sessions` tracks persistent upload state across restarts:
  1. Hash file (SHA-256) and look up active session by composite key `(page_id, file_hash, file_size_bytes)` where `status = 'in_progress'`.
  2. If session exists, query Meta `GET https://graph-video.facebook.com/v26.0/<session_id>` for confirmed offset. If query fails or returns non-numeric offset, **fail closed** and abort with descriptive error.
  3. If fresh session, call `POST https://graph-video.facebook.com/v26.0/<page_id>/videos` with `upload_phase=start`, receive `upload_session_id` and non-empty initial canonical `video_id`.
  4. Upload 4MB chunks with `upload_phase=transfer`, persisting confirmed offset to SQLite after every chunk.
  5. Call `POST https://graph-video.facebook.com/v26.0/<page_id>/videos` with `upload_phase=finish`, preserve canonical `video_id`, and mark session `completed`.

### 6.3 Dual-Mode Scheduling & JIT Promotion
- Meta-side: Posts scheduled 10 minutes to 30 days are scheduled directly on Meta Graph API with `published=false&scheduled_publish_time=...`.
- Local-queued: Posts >30 days stay in `local_queued`. Hourly cron promotes posts entering the `<=27 day` window to `scheduled_on_meta`.
- Worker atomically transitions row from `queued` to `publishing` before making Meta API call.
- Ambiguous responses mark post `delivery_unknown` and reconcile via `GET /<page_id>/feed` matching exact `scheduled_publish_time` timestamp and SHA-256 message hash before any retry.

---

## 7. Comment Automation & Private Reply Engine

### 7.1 Two-Phase Lease Lock & Concurrency Control
- Worker claims unreplied comments with a 5-minute lease:
  ```sql
  UPDATE facebook_comment_logs 
  SET owner_id = ?1, claim_deadline = datetime('now', '+5 minutes'), private_reply_status = 'processing'
  WHERE comment_id = ?2 
    AND (private_reply_status IN ('none', 'failed') OR (private_reply_status = 'processing' AND claim_deadline < datetime('now')))
    AND private_reply_status != 'success';
  ```
- In-flight heartbeat loop extends `claim_deadline` by 5 minutes every 2 minutes while HTTP request is in progress.
- If Send API response is ambiguous/timed out, mark `delivery_unknown`. Reconciliation worker checks Meta thread before retrying:
  - If `status == "replied"`, marks `success`.
  - If positive confirmation of non-delivery and `retry_count < 3`, transitions to `failed`.
  - On any inconclusive error or rate limit, **fails closed** remaining in `delivery_unknown` to ensure strictly **one private reply per comment**.

---

## 8. Lead Ads Ingestion & User Data Deletion Compliance

### 8.1 Instant Lead Ads Ingestion
- Webhook delivers `leadgen_id` -> Worker calls `GET https://graph.facebook.com/v26.0/<leadgen_id>?access_token=<page_token>`.
- Decrypts field data, maps to `leads` table with `leadgen_id` uniqueness, creates activity log, and surfaces real-time notification.

### 8.2 Meta User Data Deletion Callback & Anonymization Lifecycle
- Callback endpoint `POST /meta-data-deletion` parses `signed_request`, validates HMAC-SHA256 with `META_APP_SECRET`.
- Persists request in `data_deletion_requests` with a unique `confirmation_code` (UUIDv4).
- Dispatches `meta_device_deletion_tasks` ONLY to tenants whose connected Pages have received events from that user.
- Purges user events from `meta_webhook_buffer`.
- Desktop sync worker polls pending device tasks, executes scoped local anonymization:
  - Anonymizes matching leads (`leads.name = 'Deleted User'`).
  - Anonymizes messages (`messages.text_content = '[Deleted as per privacy request]'`).
  - Deletes physical attachment spool files and database attachment rows.
  - Anonymizes `channel_identities` for `channel = 'messenger' AND identity_namespace = 'psid'`.
  - Acknowledges task completion: `UPDATE meta_device_deletion_tasks SET status = 'completed', completed_at = now()`.
- Public Status Endpoint: `GET https://rustsale.trantrongtri.com/deletion-status?id=<confirmation_code>` returns JSON `{ "confirmation_code": "...", "status": "completed" | "processing" }` where `completed` requires all dispatched tasks to be confirmed.

---

## 9. Requirement Ledger & Verification Matrix

| Requirement ID | Module | Description | Implementation Target | Verification Method |
| :--- | :--- | :--- | :--- | :--- |
| **REQ-FB-01** | Auth | Zero-secret OAuth flow with PKCE, session initiation & 60s single-use handoff code | Edge Function + Deep Link | Unit + Integration Test |
| **REQ-FB-02** | Security | AES-256-GCM local storage with OS Keyring master key | `StrongholdKeyring` | Cargo Test |
| **REQ-FB-03** | Webhooks | HMAC-SHA256 verification & raw body retry guard | `/meta-webhook` | Signature Invalidation Test |
| **REQ-FB-04** | Sync | Isolated multi-stream cursors (Messenger, Feed, Leadgen) with attachment index | `FacebookSyncWorker` | Interleaved Stream Test |
| **REQ-FB-05** | Migration | Transaction-safe V1->V3 migration with exact RAII PragmaGuard & foreign key check | `db.rs` + `migration_v2.rs` | Populated Migration Test |
| **REQ-FB-06** | Policy | 24h interactive countdown & Message Tag selection gate | Svelte Composer | Rune Reactivity Test |
| **REQ-FB-07** | Scheduler | Dual-mode scheduling (Meta 10m-30d, Local >30d JIT) with multi-attribute reconciliation | `PostScheduler` | Time-Travel Mock Test |
| **REQ-FB-08** | Media | Resumable chunked video & Reels upload with session state | `ResumableUploader` | Restart Resume Test |
| **REQ-FB-09** | Comments | Private Reply with 5-min lease lock, heartbeat & fail-closed 1-reply guarantee | `CommentWorker` | Concurrency Stress Test |
| **REQ-FB-10** | Compliance | Meta Data Deletion callback, tenant-scoped device task queue, anonymization & status API | `/meta-data-deletion` | Signed Request Test |
