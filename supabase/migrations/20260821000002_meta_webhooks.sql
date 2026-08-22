-- Durable Webhook Relay & Canonical Ingestion (revised plan Task 2)
-- meta_user_page_interactions intentionally lives in 20260821000001_meta_oauth.sql
-- because the OAuth callback is its authoritative writer.

CREATE TABLE IF NOT EXISTS meta_webhook_buffer (
    id BIGSERIAL PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    page_id TEXT NOT NULL,
    field TEXT NOT NULL,
    event_key TEXT NOT NULL,
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, page_id, event_key)
);
ALTER TABLE meta_webhook_buffer ENABLE ROW LEVEL SECURITY;

-- Explicit least-privilege grants (current Supabase projects do NOT auto-expose
-- new tables to the Data API). Tenants may read their own events and flip
-- status forward through the column-scoped UPDATE; every other column is
-- immutable to clients, keeping payloads tamper-proof.
REVOKE ALL ON TABLE meta_webhook_buffer FROM PUBLIC, anon, authenticated;
GRANT SELECT, UPDATE(status) ON TABLE meta_webhook_buffer TO authenticated;
GRANT ALL ON TABLE meta_webhook_buffer TO service_role;

CREATE POLICY "Tenants select own buffered events" ON meta_webhook_buffer
    FOR SELECT TO authenticated
    USING (auth.uid() = tenant_id);

CREATE POLICY "Tenants ack own buffered events" ON meta_webhook_buffer
    FOR UPDATE TO authenticated
    USING (auth.uid() = tenant_id)
    WITH CHECK (auth.uid() = tenant_id AND status IN ('processing', 'completed', 'failed'));

CREATE TABLE IF NOT EXISTS meta_lead_page_interactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    page_id TEXT NOT NULL,
    leadgen_id TEXT NOT NULL,
    form_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, page_id, leadgen_id)
);
CREATE INDEX IF NOT EXISTS idx_meta_lead_interactions_page ON meta_lead_page_interactions(tenant_id, page_id);
ALTER TABLE meta_lead_page_interactions ENABLE ROW LEVEL SECURITY;

-- Written exclusively inside the ingestion RPC below; no client surface in P1.
REVOKE ALL ON TABLE meta_lead_page_interactions FROM PUBLIC, anon, authenticated;

-- Atomic batch ingestion: buffer events (idempotent on
-- tenant/page/event_key), upsert ASID->PSID interaction freshness, and record
-- lead-gen identities in ONE transaction. SERVICE-ROLE ONLY: the edge
-- function resolves ownership from meta_tenant_pages before calling.
CREATE OR REPLACE FUNCTION ingest_meta_webhook_batch(
    p_tenant_id UUID,
    p_page_id TEXT,
    p_events JSONB,
    p_interactions JSONB,
    p_lead_interactions JSONB DEFAULT '[]'::JSONB
)
RETURNS INTEGER
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
    v_inserted_count INTEGER := 0;
    v_event RECORD;
    v_inter RECORD;
    v_lead RECORD;
BEGIN
    IF p_tenant_id IS NULL THEN
        RAISE EXCEPTION 'tenant id required';
    END IF;
    IF jsonb_typeof(p_events) <> 'array'
        OR jsonb_typeof(p_interactions) <> 'array'
        OR jsonb_typeof(p_lead_interactions) <> 'array' THEN
        RAISE EXCEPTION 'p_events, p_interactions and p_lead_interactions must be JSON arrays';
    END IF;

    FOR v_event IN SELECT * FROM jsonb_to_recordset(p_events) AS (field TEXT, event_key TEXT, payload JSONB)
    LOOP
        INSERT INTO public.meta_webhook_buffer (tenant_id, page_id, field, event_key, payload)
        VALUES (p_tenant_id, p_page_id, v_event.field, v_event.event_key, v_event.payload)
        ON CONFLICT (tenant_id, page_id, event_key) DO NOTHING;
        IF FOUND THEN
            v_inserted_count := v_inserted_count + 1;
        END IF;
    END LOOP;

    FOR v_inter IN SELECT * FROM jsonb_to_recordset(p_interactions) AS (user_id TEXT, app_user_id TEXT, "timestamp" TIMESTAMPTZ)
    LOOP
        INSERT INTO public.meta_user_page_interactions (tenant_id, page_id, app_user_id, page_scoped_user_id, last_interaction_at)
        VALUES (p_tenant_id, p_page_id, v_inter.app_user_id, v_inter.user_id, v_inter."timestamp")
        ON CONFLICT (tenant_id, page_id, page_scoped_user_id)
        DO UPDATE SET
            app_user_id = COALESCE(EXCLUDED.app_user_id, public.meta_user_page_interactions.app_user_id),
            last_interaction_at = GREATEST(public.meta_user_page_interactions.last_interaction_at, EXCLUDED.last_interaction_at);
    END LOOP;

    FOR v_lead IN SELECT * FROM jsonb_to_recordset(p_lead_interactions) AS (leadgen_id TEXT, form_id TEXT, created_at TIMESTAMPTZ)
    LOOP
        INSERT INTO public.meta_lead_page_interactions (tenant_id, page_id, leadgen_id, form_id, created_at)
        VALUES (p_tenant_id, p_page_id, v_lead.leadgen_id, v_lead.form_id, v_lead.created_at)
        ON CONFLICT (tenant_id, page_id, leadgen_id) DO NOTHING;
    END LOOP;

    RETURN v_inserted_count;
END;
$$;

REVOKE ALL ON FUNCTION ingest_meta_webhook_batch(UUID, TEXT, JSONB, JSONB, JSONB) FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION ingest_meta_webhook_batch(UUID, TEXT, JSONB, JSONB, JSONB) TO service_role;
