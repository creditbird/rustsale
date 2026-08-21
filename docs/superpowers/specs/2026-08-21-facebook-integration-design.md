# Facebook Integration (Messenger, Scheduler, Automations & Compliance) Design Specification

## 1. Executive Summary & Architectural Invariants

The Facebook Integration module provides enterprise-grade, multi-channel capabilities for RustSale CRM across three operational phases:
1. **Phase 1:** Zero-Secret PKCE OAuth Token Exchange with atomic session state machine, server-held encryption key for at-rest payload protection, Local Token Vault (AES-256-GCM + OS Keyring), Durable Webhook Ingestion Buffer (supporting both `entry.messaging` and `entry.changes`) with atomic server-side persistence RPC, canonical deterministic JSON hashing, and HMAC-SHA256, Isolated Multi-Stream Messenger 2-Way Sync Engine matching repository schema (`channel_conversations`, `conversation_participants`, `messages`, `message_attachments`, `message_reactions`, `message_receipts`, `pending_message_reactions`), Connected-Component Lossless Migration (V1 -> V4), and Svelte 5 Unified Inbox with 24-Hour Policy Gate & Message Tag selector.
2. **Phase 2:** Multi-Type Post Scheduler (Status, Link, Photo, Multi-Photo, Resumable Video & Reels) with Dual-Mode scheduling (Meta-side 10m–30d & Local-queued >30d), per-post credential resolution, bounded-memory streaming chunked uploader, restart-resumable upload sessions with exact server offset verification, multi-page fan-out, expired publishing lease sweeper, multi-endpoint reconciliation with Graph status-type matching, and Svelte 5 Live Feed Preview (Desktop & Mobile).
3. **Phase 3:** Marketing & Comment Automations (Auto-Like, Public Reply, 1-reply Private Send API with atomic 5-minute lease lock, permanent/transient error classification & fail-closed reconciliation), Facebook Lead Ads ingestion with custom fields, durable interaction index & isolated cursor worker, Data Deletion Compliance Callback with Durable Interaction Mapping, Registered Multi-Device Task Distribution with active-device filtering, Atomic Task Claim & Transactional Task-Scoped Anonymizer via `pending_spool_cleanups`, and Facebook Hub UI.

---

## 2. OAuth Architecture & Token Lifecycle (Zero-Secret PKCE)

### 2.1 Protocol Flow
1. **Authorization Request:** Client generates cryptographically secure `state`, PKCE `code_verifier`, and `code_challenge = BASE64URL(SHA256(code_verifier))`. Sends `state` and `code_challenge` to `/meta-oauth-start` with user JWT.
2. **Meta Redirect:** User completes consent on Facebook. Meta redirects to `/meta-oauth-callback?code=...&state=...`.
3. **Atomic Session State Machine & Token Exchange:** Backend claims authorization session atomically (`UPDATE oauth_authorization_sessions SET status = 'processing', lease_deadline = now() + INTERVAL '5 minutes' WHERE state = p_state AND (status = 'pending' OR (status = 'processing' AND lease_deadline < now())) ...`). Exchanges authorization code for short-lived token, upgrades to 60-day long-lived token, and fetches managed Pages via `GET /me/accounts`. Validates all Meta HTTP responses strictly.
4. **Encrypted Exchange Code:** Server encrypts Page tokens payload using PostgreSQL `pgp_sym_encrypt(pages::text, current_setting('app.settings.meta_encryption_key', true))` using a server-held secret key (never the client code challenge) and writes to `oauth_exchange_codes` with 60-second TTL.
5. **Desktop Claim:** Client invokes `/meta-oauth-exchange` with user JWT, `code`, and `code_verifier`.
6. **Secure RPC:** Function `complete_meta_oauth_exchange` derives tenant identity directly from `auth.uid()`, validates that `SHA256(code_verifier) == code_challenge`, decrypts payload with the server-held key, consumes and deletes payload atomically, and maps Page ownership to `meta_tenant_pages`.

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

    -- Fetch server-held encryption key from configuration (FAIL-CLOSED)
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

    -- Atomically consume and delete payload
    DELETE FROM public.oauth_exchange_codes WHERE code = p_code;

    -- Decrypt ciphertext using server-held key
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

---

## 3. Database Schema & Migration Invariants (V1 -> V4)

### 3.1 Local SQLite Tables & Indexes (`src-tauri/src/conversation/schema.sql`)

```sql
-- Upload Sessions for Resumable Video and Reels
CREATE TABLE IF NOT EXISTS facebook_upload_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    post_id INTEGER NOT NULL REFERENCES facebook_scheduled_posts(id) ON DELETE CASCADE,
    page_id TEXT NOT NULL,
    upload_type TEXT NOT NULL DEFAULT 'video' CHECK (upload_type IN ('video', 'reel')),
    upload_session_id TEXT NOT NULL UNIQUE,
    video_id TEXT NOT NULL,
    file_path TEXT NOT NULL,
    file_hash TEXT NOT NULL,
    file_size_bytes INTEGER NOT NULL,
    start_offset INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'in_progress' CHECK (status IN ('in_progress', 'finishing', 'completed', 'failed')),
    owner_id TEXT,
    lease_deadline TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_fb_upload_active_unique
ON facebook_upload_sessions(post_id, page_id, upload_type, file_hash, file_size_bytes)
WHERE status IN ('in_progress', 'finishing');

-- Scheduled Posts
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

-- Comment Automations & Audit Logs
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

### 3.2 Connected-Component Deduplication Invariant
When merging duplicate attachments:
1. Connected components are constructed across both `remote_url` and `provider_attachment_id` using Disjoint Set Union (DSU).
2. The survivor row is `MIN(id)` within the component.
3. First non-null scalar is chosen with documented survivor precedence.
4. JSON metadata keys are merged into an object map. On key collision between rows with different values, the duplicate's value is retained under `format!("{}_dup_{}", k, item.id)`.
5. Non-object / malformed values are preserved under `_legacy_unparsed_values`.
6. Row decoding uses `Result<Vec<_>>` with strict error propagation.
7. Executed in both V3 and V4 migrations.

---

## 4. Multi-Stream Ingestion & Messenger Synchronization

### 4.1 Isolated Scope Checkpoints
Each ingestion stream maintains an independent cursor checkpoint in `channel_sync_checkpoints (channel_account_id, scope, cursor, updated_at)`:
- `meta_webhook_messenger_cursor`: Messenger messages, echoes, deliveries, reads, reactions.
- `meta_webhook_feed_cursor`: Page comments and post reaction events.
- `meta_webhook_leadgen_cursor`: Facebook Lead Ads ingestion.

### 4.2 Contiguous Processing & Forward-Only State Machine
Events from `meta_webhook_buffer` are processed in strict ascending `id` order.
1. Required identifiers (`mid`, `psid`, `timestamp`) are validated before insertion.
2. Malformed events or events with unresolved dependencies (e.g. delivery/read for not-yet-synced message) are quarantined to `channel_sync_dead_letters` using the exact array index `item_index = idx as i64`.
3. Delivery events forward-only update `messages SET status = 'delivered' WHERE status IN ('sent', 'queued', 'pending')` (never downgrading `read`).
4. Read events update `messages SET status = 'read' WHERE provider_timestamp <= watermark AND direction = 'outbound'` (inbound messages are not marked read by customer watermark).
5. Out-of-order reactions for not-yet-ingested messages are persisted to `pending_message_reactions` with provider occurrence timestamp string. Reactions with missing/invalid timestamps (where `DateTime::from_timestamp_millis` returns `None`) are quarantined.
6. Cursor advances monotonically per processed/quarantined event via conditional update `WHERE CAST(excluded.cursor AS INTEGER) > CAST(COALESCE(channel_sync_checkpoints.cursor, '0') AS INTEGER)`.

---

## 5. Resumable Upload Pipeline (Videos & Reels)

### 5.1 Bounded Memory Streaming & Exact Offset Verification
- **Videos (`graph-video.facebook.com`):** 3-Phase resumable upload (`upload_phase=start` -> `upload_phase=transfer` -> `upload_phase=finish`). Tracks `facebook_upload_sessions`. On resume, queries Graph API video upload session for authoritative start offset. Verifies `server_offset > offset && server_offset <= file_size`. Chunked streaming via file seek avoids high memory spikes.
- **Reels (`rupload.facebook.com`):** Initialize via `POST /<page_id>/video_reels` with `upload_phase=start`. Send 4MB binary chunks to `https://rupload.facebook.com/video-reels/<video_id>` with headers `Offset` and `file_size`. On resume, query server confirmed `Offset` via `GET /video-reels/<video_id>` with Bearer auth; if failed or malformed, abort with error. After chunk upload, parse confirmed server offset and strictly verify `confirmed_offset == offset + bytes_to_read as u64`. Finalize via `POST /<page_id>/video_reels` with `upload_phase=finish`.

---

## 6. Post Scheduler, Dual-Mode Engine & Expired Lease Recovery

### 6.1 Dual-Mode Scheduling & Multi-Type Publishing
- **Target 10m–30d:** Pushed directly to Meta with `published=false` and `scheduled_publish_time`.
- **Target >30d:** Held locally in SQLite with `scheduling_mode='local_queued'`. Hourly background worker promotes posts entering `<=27 days`. Resolves token and Page ID per claimed post row.
- **Supported Post Types:**
  - Status: `POST /<page_id>/feed` (`message`)
  - Link: `POST /<page_id>/feed` (`message`, `link`)
  - Photo: `POST /<page_id>/photos` (`caption`, multipart `source`)
  - Multi-Photo: Upload photos with `published=false` -> `POST /<page_id>/feed` with `attached_media`
  - Video & Reel: Chunked resumable streaming via `FacebookUploader`

### 6.2 Atomic Lease Claims & Expired Sweeper
- Promotion claims row: `UPDATE ... SET status = 'publishing', owner_id = ?1, claim_deadline = datetime('now', '+5 minutes') WHERE id = ?2 AND status = 'queued'`.
- Expired publishing sweeper moves timed-out rows (`claim_deadline < datetime('now')`) to `delivery_unknown`.
- Reconciliation cycle claims `delivery_unknown` rows with atomic lease lock. Queries both `/feed` and `/scheduled_posts`, matching post type against Graph `status_type`, message hash, and publication timestamp within 60s window. Sets `status = 'published'` for `/feed` matches and `status = 'scheduled_on_meta'` for `/scheduled_posts`. If inconclusive, clears owner and deadline so future cycles can retry (fail-closed).

---

## 7. Comment Automations, Atomic Claim & Fail-Closed Reconciliation

1. **Loop Guard:** Skips if `sender_id == page_id`.
2. **7-Day Window:** Skips if comment age > 7 days.
3. **Atomic 1-Reply Claim:** `facebook_comment_logs` uniqueness on `comment_id` with 5-minute lease.
4. **Permanent vs Transient Errors:** Permanent 4xx errors mark `failed` without retries. Transient 5xx / timeouts mark `delivery_unknown`.
5. **Fail-Closed Reconciliation:** Reconciler calls `GET /<comment_id>?fields=private_reply_status`. Terminal transitions strictly enforce owner ID (`replied` -> `success`, `can_reply` -> `failed`).

---

## 8. Data Deletion Compliance Lifecycle & Transactional Anonymizer

1. **Registered Devices Inventory & Durable Tombstones:** `tenant_registered_devices (tenant_id, device_id, device_name, last_seen_at)` and `meta_tenant_deletion_tombstones (tenant_id, request_id, page_id, user_id)`. Tasks are fanned out to all registered tenant devices; durable tombstones ensure reconnect scrubbing for offline or newly registered devices.
2. **Durable Interaction Index:** `meta_user_page_interactions` and `meta_lead_page_interactions` track interactions across pages and Lead Ads.
3. **Callback Endpoint (`/meta-data-deletion`):** Verifies HMAC `signed_request`, executes PostgreSQL transactional RPC `create_data_deletion_request_and_tasks` (accessible only to service_role) which purges buffered events across all shapes, persists durable tenant tombstones, and creates `meta_device_deletion_tasks` for all registered tenant devices (with reconnect scrub backfill).
4. **Desktop Worker Lifecycle:**
   - Desktop worker claims task: `PATCH ... status = 'acknowledged'` sending `anon_key` as `apikey` and `user_jwt` in `Authorization`.
   - Transactionally redacts messages, deletes attachments, and registers task-scoped paths in `pending_spool_cleanups(task_id, local_path)`.
   - Commits transaction, then sweeps task-scoped cleanups.
   - Updates task status to `completed` on Supabase only when all task-scoped spool files are confirmed removed from disk and PATCH returns HTTP 200.
5. **Status Endpoint (`/deletion-status`):** Reports `completed` only when all required device tasks are completed.
