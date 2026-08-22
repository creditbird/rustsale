-- Zero-Secret PKCE OAuth & Session State Machine (revised plan Task 1)
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
GRANT SELECT ON TABLE meta_tenant_pages TO authenticated;

CREATE TABLE IF NOT EXISTS oauth_authorization_sessions (
    state TEXT PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    code_challenge TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'consumed', 'failed')),
    attempts INTEGER NOT NULL DEFAULT 0,
    -- Fencing token: generated per successful claim; every terminal update
    -- must present the CURRENT lease_id so an expired worker cannot clobber
    -- its replacement.
    lease_id UUID,
    lease_deadline TIMESTAMPTZ,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
ALTER TABLE oauth_authorization_sessions ENABLE ROW LEVEL SECURITY;

-- Explicit least-privilege grants: current Supabase projects do NOT auto-expose
-- new tables to the Data API. Only the service-role insert performed by
-- /meta-oauth-start is granted; everything else stays server-side.
GRANT INSERT ON TABLE oauth_authorization_sessions TO service_role;

-- Server-write-only: session lifecycle (claim/finalize/fail) happens through
-- the SECURITY DEFINER RPCs below so callers cannot forge status, attempts,
-- lease fields, or expiry.
REVOKE ALL ON oauth_authorization_sessions FROM PUBLIC, anon, authenticated;

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
REVOKE ALL ON oauth_exchange_codes FROM PUBLIC, anon, authenticated;

-- Authoritative ASID -> PSID identity mapping captured during OAuth.
-- Table lives here (not in the webhook migration) because the OAuth callback
-- is its authoritative writer.
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

CREATE POLICY "Tenants read own interactions" ON meta_user_page_interactions
    FOR SELECT TO authenticated
    USING (auth.uid() = tenant_id);
GRANT SELECT ON TABLE meta_user_page_interactions TO authenticated;

-- Atomic lease claim: real SQL predicate (PostgREST cannot express this),
-- increments attempts, mints a fresh fence token.
CREATE OR REPLACE FUNCTION claim_meta_oauth_session(p_state TEXT)
RETURNS JSONB
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
    v_row RECORD;
BEGIN
    UPDATE public.oauth_authorization_sessions
       SET status = 'processing',
           attempts = attempts + 1,
           lease_id = gen_random_uuid(),
           lease_deadline = now() + INTERVAL '5 minutes',
           updated_at = now()
     WHERE state = p_state
       AND expires_at > now()
       AND (
             status = 'pending'
          OR (status = 'processing' AND lease_deadline < now())
           )
    RETURNING tenant_id, code_challenge, lease_id INTO v_row;

    IF v_row IS NULL THEN
        RAISE EXCEPTION 'session_not_claimable';
    END IF;

    RETURN jsonb_build_object(
        'tenant_id', v_row.tenant_id,
        'code_challenge', v_row.code_challenge,
        'lease_id', v_row.lease_id
    );
END;
$$;
REVOKE ALL ON FUNCTION claim_meta_oauth_session(TEXT) FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION claim_meta_oauth_session(TEXT) TO service_role;

-- Fenced failure transition: only the CURRENT lease holder may fail a session.
CREATE OR REPLACE FUNCTION fail_meta_oauth_session(p_state TEXT, p_lease_id UUID)
RETURNS VOID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
    UPDATE public.oauth_authorization_sessions
       SET status = 'failed',
           lease_id = NULL,
           lease_deadline = NULL,
           updated_at = now()
     WHERE state = p_state
       AND lease_id = p_lease_id
       AND status = 'processing';
END;
$$;
REVOKE ALL ON FUNCTION fail_meta_oauth_session(TEXT, UUID) FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION fail_meta_oauth_session(TEXT, UUID) TO service_role;

-- Transactional success path: persists the COMPLETE validated interaction
-- set, creates the single-use encrypted exchange code, and fences the
-- session to consumed — all or nothing. Called only after the edge function
-- has fully validated 100% ASID->PSID coverage in memory.
CREATE OR REPLACE FUNCTION finalize_meta_oauth(
    p_state TEXT,
    p_lease_id UUID,
    p_interactions JSONB,
    p_code TEXT,
    p_pages_json TEXT
)
RETURNS VOID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
    v_tenant_id UUID;
    v_code_challenge TEXT;
    v_server_key TEXT;
    v_interaction RECORD;
BEGIN
    -- Fence: exactly the current lease holder may finalize.
    UPDATE public.oauth_authorization_sessions
       SET status = 'consumed',
           lease_id = NULL,
           lease_deadline = NULL,
           updated_at = now()
     WHERE state = p_state
       AND lease_id = p_lease_id
       AND status = 'processing'
       AND expires_at > now()
    RETURNING tenant_id, code_challenge INTO v_tenant_id, v_code_challenge;

    IF v_tenant_id IS NULL THEN
        RAISE EXCEPTION 'stale_or_unknown_lease';
    END IF;

    -- Persist the full validated ASID->PSID mapping set.
    FOR v_interaction IN
        SELECT * FROM jsonb_to_recordset(p_interactions)
            AS (page_id TEXT, app_user_id TEXT, page_scoped_user_id TEXT)
    LOOP
        INSERT INTO public.meta_user_page_interactions
            (tenant_id, page_id, app_user_id, page_scoped_user_id, last_interaction_at)
        VALUES (v_tenant_id, v_interaction.page_id, v_interaction.app_user_id,
                v_interaction.page_scoped_user_id, now())
        ON CONFLICT (tenant_id, page_id, page_scoped_user_id)
        DO UPDATE SET
            app_user_id = COALESCE(EXCLUDED.app_user_id,
                                   meta_user_page_interactions.app_user_id),
            last_interaction_at = GREATEST(meta_user_page_interactions.last_interaction_at,
                                           EXCLUDED.last_interaction_at);
    END LOOP;

    -- Single-use encrypted handoff payload (60 second TTL), PKCE-bound.
    v_server_key := current_setting('app.settings.meta_encryption_key', true);
    IF v_server_key IS NULL OR v_server_key = '' THEN
        RAISE EXCEPTION 'Missing configuration: app.settings.meta_encryption_key is required';
    END IF;

    INSERT INTO public.oauth_exchange_codes (code, tenant_id, code_challenge, payload, expires_at)
    VALUES (
        p_code,
        v_tenant_id,
        v_code_challenge,
        extensions.pgp_sym_encrypt(p_pages_json, v_server_key),
        now() + INTERVAL '60 seconds'
    );
END;
$$;
REVOKE ALL ON FUNCTION finalize_meta_oauth(TEXT, UUID, JSONB, TEXT, TEXT) FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION finalize_meta_oauth(TEXT, UUID, JSONB, TEXT, TEXT) TO service_role;

-- Desktop-side completion: derives tenant from auth.uid(), binds the payload
-- to the original PKCE challenge, decrypts with the server-held key, upserts
-- page ownership, and burns the code atomically.
CREATE OR REPLACE FUNCTION complete_meta_oauth_exchange(
    p_code TEXT,
    p_code_verifier TEXT
)
RETURNS JSONB
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
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
    v_expected_challenge := rtrim(replace(replace(encode(extensions.digest(p_code_verifier, 'sha256'), 'base64'), '+', '-'), '/', '_'), '=');

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
    v_pages := extensions.pgp_sym_decrypt(v_payload_bytes, v_server_key)::JSONB;

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
