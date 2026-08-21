# Facebook Integration (Messenger, Scheduler, Automations & Compliance) Design Specification

## 1. Executive Summary & Architectural Invariants

The Facebook Integration module provides enterprise-grade, multi-channel capabilities for RustSale CRM across three operational phases:
1. **Phase 1:** Zero-Secret PKCE OAuth Token Exchange, Local Token Vault (AES-256-GCM + OS Keyring), Durable Webhook Ingestion Buffer with HMAC-SHA256, Isolated Multi-Stream Messenger 2-Way Sync Engine matching repository schema (`channel_conversations`, `conversation_participants`, `messages`, `message_attachments`, `message_reactions`, `message_receipts`, `pending_message_reactions`), and Svelte 5 Unified Inbox with 24-Hour Policy Gate & Message Tag selector.
2. **Phase 2:** Multi-Type Post Scheduler (Status, Link, Photo, Multi-Photo, Resumable Video & Reels) with Dual-Mode scheduling (Meta-side 10m–30d & Local-queued >30d), restart-resumable upload sessions with server offset verification, multi-page fan-out, expired publishing lease sweeper, and Svelte 5 Live Feed Preview (Desktop & Mobile).
3. **Phase 3:** Marketing & Comment Automations (Auto-Like, Public Reply, 1-reply Private Send API with active heartbeat loop & fail-closed reconciliation), Facebook Lead Ads ingestion with custom fields, Data Deletion Compliance Callback with Durable Interaction Mapping, Tenant/Page Scoped Device Task Dispatch & Transactional Anonymizer via `pending_spool_cleanups`, and Facebook Hub UI.

---

## 2. OAuth Architecture & Token Lifecycle (Zero-Secret PKCE)

### 2.1 Protocol Flow
1. **Authorization Request:** Client generates cryptographically secure `state`, PKCE `code_verifier`, and `code_challenge = BASE64URL(SHA256(code_verifier))`. Sends `state` and `code_challenge` to `/meta-oauth-start` with user JWT.
2. **Meta Redirect:** User completes consent on Facebook. Meta redirects to `/meta-oauth-callback?code=...&state=...`.
3. **Token Exchange:** Backend exchanges authorization code for short-lived token, upgrades to 60-day long-lived token, and fetches managed Pages via `GET /me/accounts`.
4. **Single-Use Exchange Code:** Server writes Page tokens (encrypted at rest) and PKCE challenge to `oauth_exchange_codes` with 60-second TTL.
5. **Desktop Claim:** Client invokes `/meta-oauth-exchange` with user JWT, `code`, and `code_verifier`.
6. **Secure RPC:** Function `complete_meta_oauth_exchange` derives tenant identity directly from `auth.uid()`, validates PKCE challenge, consumes and deletes payload atomically, and maps Page ownership to `meta_tenant_pages`.

```sql
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
    v_payload TEXT;
    v_pages JSONB;
    v_page RECORD;
BEGIN
    v_tenant_id := auth.uid();
    IF v_tenant_id IS NULL THEN
        RAISE EXCEPTION 'Unauthorized: user JWT required';
    END IF;

    SELECT payload INTO v_payload
    FROM public.oauth_exchange_codes
    WHERE code = p_code
      AND tenant_id = v_tenant_id
      AND code_challenge = p_verifier_challenge
      AND consumed = FALSE
      AND expires_at > now()
    FOR UPDATE;

    IF v_payload IS NULL THEN
        RAISE EXCEPTION 'Invalid or expired exchange code';
    END IF;

    -- Atomically consume and delete payload
    DELETE FROM public.oauth_exchange_codes WHERE code = p_code;

    v_pages := v_payload::JSONB;

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

## 3. Database Schema & Migration Invariants

### 3.1 Local SQLite Tables & Indexes (`src-tauri/src/conversation/schema.sql`)

```sql
-- Upload Sessions for Resumable Video and Reels
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

CREATE UNIQUE INDEX IF NOT EXISTS idx_fb_upload_active_unique 
ON facebook_upload_sessions(page_id, upload_type, file_hash, file_size_bytes) 
WHERE status = 'in_progress';

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

---

## 4. Multi-Stream Ingestion & Messenger Synchronization

### 4.1 Isolated Scope Checkpoints
Each ingestion stream maintains an independent cursor checkpoint in `channel_sync_checkpoints (channel_account_id, scope, cursor, updated_at)`:
- `meta_webhook_messenger_cursor`: Messenger messages, echoes, deliveries, reads, reactions.
- `meta_webhook_feed_cursor`: Page comments and post reaction events.
- `meta_webhook_leadgen_cursor`: Facebook Lead Ads ingestion.

### 4.2 Contiguous Processing & Poison Event Isolation
Events from `meta_webhook_buffer` are processed in strict ascending `id` order.
1. Required identifiers (`mid`, `psid`, `timestamp`) are validated before insertion.
2. Malformed events are immediately quarantined to `channel_sync_dead_letters (channel_account_id, scope, event_id, item_index, ...)` with `error_reason = 'missing_required_identifier'`.
3. Out-of-order reactions for not-yet-ingested messages are persisted to `pending_message_reactions`.
4. Delivery and read events insert into `message_receipts` scoped to the exact conversation and update `messages.status`.
5. Cursor advances monotonically per processed/quarantined event via conditional update `WHERE CAST(excluded.cursor AS INTEGER) > CAST(COALESCE(channel_sync_checkpoints.cursor, '0') AS INTEGER)`.

---

## 5. Resumable Upload Pipeline (Videos & Reels)

### 5.1 Protocol Differences
- **Videos (`graph-video.facebook.com`):** 3-Phase resumable upload (`upload_phase=start` -> `upload_phase=transfer` -> `upload_phase=finish`). Query server offset via `GET /<upload_session_id>`.
- **Reels (`rupload.facebook.com`):** Initialize via `POST /<page_id>/video_reels` with `upload_phase=start`. Send 4MB binary chunks to `https://rupload.facebook.com/video-reels/<video_id>` with headers `Offset` and `file_size`. On resume, query server confirmed `Offset` via `GET /video-reels/<video_id>` with Bearer auth; if failed or malformed, abort with error. Finalize via `POST /<page_id>/video_reels` with `upload_phase=finish`.

---

## 6. Post Scheduler, Dual-Mode Engine & Expired Lease Recovery

### 6.1 Dual-Mode Scheduling
- **Target 10m–30d:** Pushed directly to Meta with `published=false` and `scheduled_publish_time`.
- **Target >30d:** Held locally in SQLite with `scheduling_mode='local_queued'`. Hourly background worker promotes posts entering `<=27 days`.

### 6.2 Atomic Lease Claims & Expired Sweeper
- Promotion claims row: `UPDATE ... SET status = 'publishing', owner_id = ?1, claim_deadline = datetime('now', '+5 minutes') WHERE id = ?2 AND status = 'queued'`.
- Wraps all media reads and uploads in a publisher block so all errors become `publish_result` and execute the ownership-guarded transition to `delivery_unknown`.
- Expired publishing sweeper moves timed-out rows (`claim_deadline < datetime('now')`) to `delivery_unknown`.
- Reconciliation cycle claims `delivery_unknown` rows with atomic lease lock. Searches Meta Feed for matching message hash and timestamp. If found, marks `scheduled_on_meta`. If inconclusive or rate-limited, remains strictly in `delivery_unknown` (fail-closed).

---

## 7. Comment Automations, Heartbeat & Fail-Closed Reconciliation

1. **Loop Guard:** Skips if `sender_id == page_id`.
2. **7-Day Window:** Skips if comment age > 7 days.
3. **Atomic 1-Reply Claim:** `facebook_comment_logs` uniqueness on `comment_id` with 5-minute lease.
4. **Active Heartbeat Loop:** Tokio task renews `claim_deadline = datetime('now', '+5 minutes')` every 120s via background channel while HTTP call awaits, cancelling immediately upon completion.
5. **Fail-Closed Reconciliation:** On transport timeout or 5xx error, sets `delivery_unknown`. Reconciler calls `GET /<comment_id>?fields=private_reply_status`. Only transitions out on definitive response (`replied` -> `success`, `can_reply` -> `failed`).

---

## 8. Data Deletion Compliance Lifecycle & Transactional Anonymizer

1. **Durable Interaction Index:** `meta_user_page_interactions (tenant_id, page_id, user_id, last_interacted_at, PRIMARY KEY(tenant_id, page_id, user_id))` tracks user interactions across all channels and pages.
2. **Callback Endpoint (`/meta-data-deletion`):** Verifies HMAC `signed_request`, queries interaction index, and transactionally creates `data_deletion_requests` with `required_tasks_count` and `meta_device_deletion_tasks` scoped to `(request_id, tenant_id, page_id, user_id)`.
3. **Transactional Desktop Anonymizer:**
   - Transactionally updates `leads`, queues attachment paths into `pending_spool_cleanups`, deletes `message_attachments`, updates `messages SET text_content = '[Deleted as per privacy request]', edited_at = datetime('now')`, deletes receipts, reactions, and pending reactions, anonymizes `facebook_comment_logs`, and pseudonymizes `channel_identities`.
   - Commits transaction, then sweeps `pending_spool_cleanups` physically deleting disk files. Deletes queue row only on successful deletion or `NotFound`.
   - Acknowledges task completion.
4. **Status Endpoint (`/deletion-status`):** Reports `completed` only when all required device tasks are completed.
