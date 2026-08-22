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

CREATE POLICY "Tenants create own auth sessions" ON oauth_authorization_sessions
    FOR INSERT WITH CHECK (auth.uid() = tenant_id);

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

CREATE OR REPLACE FUNCTION create_meta_oauth_exchange_code(
    p_code TEXT,
    p_tenant_id UUID,
    p_code_challenge TEXT,
    p_pages_json TEXT
)
RETURNS VOID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, auth, pg_temp
AS $$
DECLARE
    v_server_key TEXT;
BEGIN
    -- Derive server-held encryption key from PostgreSQL configuration / vault (FAIL-CLOSED)
    v_server_key := current_setting('app.settings.meta_encryption_key', true);
    IF v_server_key IS NULL OR v_server_key = '' THEN
        RAISE EXCEPTION 'Missing configuration: app.settings.meta_encryption_key is required';
    END IF;

    INSERT INTO public.oauth_exchange_codes (code, tenant_id, code_challenge, payload, expires_at)
    VALUES (
        p_code,
        p_tenant_id,
        p_code_challenge,
        pgp_sym_encrypt(p_pages_json, v_server_key),
        now() + INTERVAL '60 seconds'
    );
END;
$$;

REVOKE ALL ON FUNCTION create_meta_oauth_exchange_code FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION create_meta_oauth_exchange_code TO service_role;

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
