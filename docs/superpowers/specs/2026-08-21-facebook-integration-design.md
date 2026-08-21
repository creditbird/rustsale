# Facebook Integration Design Specification — RustSale v2

**Date:** 2026-08-21  
**Status:** Production Ready Specification (Remediated for GPT-5.6 Sol / Fable 5 Production Standards)  
**Author / Reviewer:** Antigravity Team & Lead Architect  
**Target Platform:** Tauri 2.0 (Rust) + Svelte 5 (Runes) + SQLite Local + Supabase (Auth, RLS, Edge Functions)

---

## 1. Executive Summary & Architecture Overview

The Facebook Integration module empowers RustSale v2 with enterprise-grade Meta capabilities across three pillars:
1. **Unified Omnichannel Messenger Inbox (`/conversation`)**: Seamless 2-way real-time messaging with Facebook Pages, integrating directly into the existing RustSale unified inbox alongside Zalo and Email.
2. **Facebook Post Scheduler & Planner (`/facebook`)**: Dual-mode scheduling (Meta-side 10m–30d & Local-queued >30d) supporting Status, Link, Single Photo, Multi-photo Albums, Resumable Videos, and Reels with live Desktop/Mobile feed previews.
3. **Automations & Growth Engine**: Compliant Private Reply for comments, 24-hour window countdown with Message Tag gating, Lead Ads instant CRM ingestion, and proactive Page Health monitoring.

### High-Level Architecture

```mermaid
flowchart TD
    subgraph Meta_Cloud ["Meta Cloud Platform (Graph API v26.0+)"]
        FB_User["Khách hàng Facebook"]
        FB_Send["Send API / Feed API / Video Graph API"]
        FB_Hook["Meta Webhooks (messages, feed, leadgen)"]
    end

    subgraph Supabase_Cloud ["Supabase Cloud Relay & Auth Bridge"]
        EdgeOAuth["Edge Function: /meta-oauth-callback (Verify & Exchange)"]
        EdgeExchange["Edge Function: /meta-oauth-exchange (1-Time Secure Handoff)"]
        EdgeDebug["Edge Function: /meta-debug-token (Inspect Scopes)"]
        EdgeHook["Edge Function: /meta-webhook (HMAC Raw Body Verify & Retry Guard)"]
        TenantMap[("meta_tenant_pages (Service-Role Protected)")]
        BufferDB[("meta_webhook_buffer (Immutable Log + RLS)")]
        Realtime["Supabase Realtime (Tenant Scoped Private Channel)"]
    end

    subgraph Desktop_App ["RustSale Desktop App (Local-First Native)"]
        DeepLink["tauri-plugin-deep-link (rustsale://auth/facebook?exchange_code=...)"]
        Stronghold["Tauri Stronghold / OS Keyring (AES-256 Authenticated Local Storage)"]
        RustSyncWorker["Rust Inbound Sync Worker (Multi-Event Catch-up + Cursor)"]
        RustOutbox["Rust Outbox Sender (Exponential Backoff + Typed Error Classifier)"]
        SQLite[("Local SQLite DB (channel_accounts, messages, posts, leads)")]
        MediaCache["Local Media Cache (Sanitized / Attachments)"]
        SvelteUI["Svelte 5 Runes UI (/conversation, /facebook)"]
    end

    %% Auth Flow
    SvelteUI -->|Bấm Kết nối Page| DeepLink
    EdgeOAuth -->|Store 60s Single-Use Code| EdgeExchange
    DeepLink -->|Call with JWT & Exchange Code| EdgeExchange
    EdgeExchange -->|Return Encrypted Tokens| Stronghold

    %% Webhook Flow
    FB_User -->|Nhắn tin / Comment| FB_Hook
    FB_Hook -->|POST with X-Hub-Signature-256| EdgeHook
    EdgeHook -->|Unpack entry[] & INSERT| BufferDB
    EdgeHook -.->|500 on DB Error (Meta Retries)| FB_Hook
    BufferDB -->|Broadcast Event| Realtime
    Realtime -.->|Live Event| RustSyncWorker
    RustSyncWorker -->|Cursor-Based Pull WHERE id > last_cursor| SQLite
    RustSyncWorker -->|Tải Media Cục bộ| MediaCache
    SQLite -->|Reactive Timeline| SvelteUI

    %% Outbound Flow
    SvelteUI -->|Gửi tin / Đăng bài| RustOutbox
    RustOutbox -->|POST /<page_id>/messages /feed| FB_Send
```

---

## 2. Security, Authentication & Token Lifecycle

### 2.1 Zero-Secret Desktop Architecture & Secure One-Time Handoff
- **Strict Rule:** Meta `client_secret` and App Access Tokens (`app_id|app_secret`) MUST NEVER reside within the desktop binary.
- **Authorization Code Flow with 60-Second Replay-Proof Handoff:**
  1. Desktop app triggers system browser to open Meta OAuth:
     `https://www.facebook.com/v26.0/dialog/oauth?client_id={APP_ID}&redirect_uri={EDGE_FN_CALLBACK}&state={CSRF_STATE}&scope=pages_messaging,pages_manage_posts,pages_read_engagement,pages_show_list,leads_retrieval`
  2. Meta redirects to Supabase Edge Function (`/meta-oauth-callback`).
  3. Edge Function verifies `state`, exchanges `code` for short-lived user token, and exchanges again for **Long-lived User Token** (60-day validity) using `client_secret`.
  4. Edge Function queries `GET /me/accounts` to retrieve all Page Access Tokens (never-expiring unless revoked).
  5. Edge Function writes tenant-to-page ownership mapping into `meta_tenant_pages (tenant_id UUID, page_id TEXT, page_name TEXT)` using `supabaseAdmin` (Service Role).
  6. Edge Function generates a cryptographically secure, 60-second single-use `exchange_code` (stored in `oauth_exchange_codes` table with `consumed = FALSE` and `expires_at = now() + 60s`).
  7. Edge Function redirects to Desktop App via deep-link: `rustsale://auth/facebook?exchange_code={EXCHANGE_CODE}`.
  8. Desktop App calls authenticated Edge Function `/meta-oauth-exchange` with its Supabase Auth JWT and `exchange_code`.
  9. Edge Function executes atomic consumption (`UPDATE oauth_exchange_codes SET consumed = TRUE WHERE code = ? AND tenant_id = ? AND consumed = FALSE AND expires_at > now() RETURNING payload_encrypted`), returns the encrypted Page Tokens, and prevents replay attacks.
  10. Desktop App stores decrypted Page Tokens securely in **Tauri Stronghold / OS Keyring**.

### 2.2 Token Inspection & Status Machine
- Scheduled background task calls Edge Function `/meta-debug-token` to inspect `debug_token` using server-side App Access Token.
- Token States:
  - `valid`: Token active and operational.
  - `invalid`: Token expired or malformed.
  - `revoked`: User changed password, revoked permissions, or lost admin role on Page.

---

## 3. Database Schema & SQLite Data Architecture

All SQLite database migrations enforce `PRAGMA foreign_keys = ON;`.

### 3.1 SQLite Local Database (Desktop) & Migration Path

```sql
-- 1. Bảng quản lý Token và cấu hình riêng cho từng Fanpage
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

-- 2. Chuẩn hóa channel_identities hỗ trợ Page-Scoped PSID và bảo toàn Unique cho Legacy Zalo/Email
CREATE TABLE IF NOT EXISTS channel_identities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    lead_id INTEGER,
    channel TEXT NOT NULL CHECK (channel IN ('zalo', 'whatsapp', 'messenger', 'email')),
    channel_account_id INTEGER, -- NULL cho Zalo/Email toàn cục, NOT NULL cho Facebook Page-scoped PSID
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

-- Partial Unique Indexes:
CREATE UNIQUE INDEX IF NOT EXISTS idx_channel_identities_legacy_unique 
ON channel_identities(channel, identity_namespace, external_id) 
WHERE channel_account_id IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_channel_identities_page_unique 
ON channel_identities(channel, channel_account_id, identity_namespace, external_id) 
WHERE channel_account_id IS NOT NULL;

-- 3. Bảng quản lý Lên lịch đăng bài (Post Scheduler)
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
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'queued', 'publishing', 'scheduled_on_meta', 'published', 'failed', 'overdue_prompt')),
    error_code TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_fb_scheduled_posts_status ON facebook_scheduled_posts(status, scheduled_publish_time);
CREATE INDEX IF NOT EXISTS idx_fb_scheduled_posts_group ON facebook_scheduled_posts(draft_group_id);

-- 4. Bảng cấu hình Tự động phản hồi bình luận (Comment Automation)
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

-- 5. Bảng nhật ký phản hồi comment (Idempotency, Lease Lock & Audit Log)
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

---

## 4. Webhook Relay & Inbound Synchronization Engine

### 4.1 Edge Function Webhook Handler (`/meta-webhook`)
1. **GET Verification Handshake:**
   - Evaluates `hub.mode == 'subscribe'` and `hub.verify_token == META_VERIFY_TOKEN`.
   - Returns raw `hub.challenge` string.
2. **POST Event Ingestion with Retry Guard:**
   - Reads request **raw bytes** before any JSON parsing.
   - Calculates HMAC-SHA256 with `META_APP_SECRET` and compares with `X-Hub-Signature-256` in constant time. If invalid, returns 403.
   - Unpacks `entry[]` containing multiple `messaging[]` or `changes[]`.
   - Generates deterministic `event_key`:
     - `messages` / `message_echoes` → `entry.messaging.message.mid`
     - `feed` comments → `entry.changes.value.comment_id`
     - `leadgen` → `entry.changes.value.leadgen_id`
     - `deliveries` / `reads` → `{recipient_id}_{watermark}`
     - `reactions` → `{mid}_{sender_id}_{reaction}_{action}`
   - Batch inserts records into `meta_webhook_buffer` with `ON CONFLICT (page_id, event_type, event_key) DO NOTHING`.
   - If database insertion fails, **returns HTTP 500 Internal Server Error** to force Meta to retry.
   - On success: Returns HTTP 200 OK immediately and broadcasts to `tenant:{tenant_id}`.

### 4.2 Desktop Cursor Catch-Up & Live Inbound Worker
- Desktop client maintains its own sync cursor in SQLite: `channel_sync_checkpoints (channel_account_id, scope='meta_webhook_cursor', cursor=last_synced_id)`.
- Queries `meta_webhook_buffer WHERE page_id IN (...) AND id > :last_synced_id ORDER BY id ASC LIMIT 500`.
- Atomically handles all events within a transaction and only updates `channel_sync_checkpoints` upon successful commit.

---

## 5. Outbound Messaging & 24-Hour Policy Compliance

### 5.1 Outbox Queue & Typed Error Classification
Outbox sender classifies Meta API response errors into typed results:

| Error Category | Subcodes / Error Types | Action |
| :--- | :--- | :--- |
| **Transient Errors** | `613` (Rate limit), `32` (Page throttle), Network Timeout | Exponential Backoff Retry (1s, 2s, 4s, 8s... max 5 retries). |
| **Permanent Errors** | `2018278` (Outside 24h window without tag), `551` (User blocked Page), `190` (Token invalid/revoked) | Mark `status = 'dead_letter'`, halt retries, render actionable error prompt in UI. |

### 5.2 24-Hour Window & Message Tag Gate
- **Interactive Countdown**: UI displays dynamic countdown timer based on customer's last inbound message or reaction.
- **Outside 24h Gate**:
  - When countdown expires, composer mandates selecting a valid **Message Tag**:
    1. `CONFIRMED_EVENT_UPDATE`
    2. `POST_PURCHASE_UPDATE`
    3. `ACCOUNT_UPDATE`
    4. `HUMAN_AGENT`
  - Client-side heuristic scanner flags promotional keywords (`sale`, `giảm giá`, `voucher`, `khuyến mãi`).

---

## 6. Post Scheduler & Multi-Type Media Engine

### 6.1 Dual-Mode Scheduling Specification
1. **Meta-side Scheduling (10 mins to 30 days):**
   - Creates post on Meta with `published=false` and `scheduled_publish_time={UNIX_TIMESTAMP}`.
   - Executes read-after-write verification: `GET /{remote_post_id}?fields=is_published,scheduled_publish_time` with `Authorization: Bearer <token>`.
2. **Local-queued Scheduling (> 30 days):**
   - Keeps post locally with status `queued`. Media is NOT uploaded to Meta during draft/queue phase.
   - When post time reaches `<= 29 days`, worker uploads media and transitions post to `scheduled_on_meta`.

### 6.2 Multi-Type Media Publishing Flow
- **Status / Link**: `POST /{page_id}/feed`
- **Single Photo**: `POST /{page_id}/photos` (`published=false`, `scheduled_publish_time`)
- **Multi-Photo Album**: JIT unpublished upload of each photo (`POST /{page_id}/photos?published=false`) -> Collect `photo_fbid[]` -> Call `POST /{page_id}/feed` with `attached_media`.
- **Video Post**: Resumable 3-phase via `graph-video.facebook.com` (`start` -> `transfer` -> `finish`).
- **Reel Post**: `POST /{page_id}/video_reels` (`start` -> `transfer` -> `finish`).
