-- Facebook/Meta relay schema: tenant-page mapping + durable webhook buffer
--
-- Security model (authoritative):
-- - meta_tenant_pages rows are written ONLY through the SECURITY DEFINER RPC
--   replace_meta_tenant_pages(), which Edge Functions call AFTER complete
--   Meta-side /me/accounts verification. Tenants hold SELECT-only RLS.
-- - The RPC is atomic (single transaction): takeover of stale owners,
--   revocation of lost Pages (including the zero-Page case), and upsert of
--   the verified set either fully commit or not at all.
-- - meta_webhook_buffer carries a durable tenant_id stamped at ingestion
--   time. Authorization follows the owning tenant at receive time, so a Page
--   ownership transfer can never expose the previous tenant's history.
CREATE TABLE IF NOT EXISTS meta_tenant_pages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    page_id TEXT NOT NULL UNIQUE,
    page_name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, page_id)
);
ALTER TABLE meta_tenant_pages ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Tenants view own pages" ON meta_tenant_pages
    FOR SELECT USING (auth.uid() = tenant_id);

CREATE TABLE IF NOT EXISTS meta_webhook_buffer (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    page_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    event_key TEXT NOT NULL,
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'synced', 'dead_letter')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_meta_buffer_dedup ON meta_webhook_buffer(page_id, event_type, event_key);
CREATE INDEX IF NOT EXISTS idx_meta_buffer_sync ON meta_webhook_buffer(tenant_id, status, id);

ALTER TABLE meta_webhook_buffer ENABLE ROW LEVEL SECURITY;

-- Time-stable isolation: readers see exactly the events ingested under their
-- own tenancy, regardless of later Page ownership changes.
CREATE POLICY "Tenants read own buffer rows" ON meta_webhook_buffer
    FOR SELECT USING (tenant_id = auth.uid());

-- No INSERT/UPDATE/DELETE policies exist for clients: payloads stay immutable
-- and status transitions go exclusively through the RPC below.

-- Atomic, server-only reconciliation. Called by /meta-oauth with the FULLY
-- verified Page set (possibly empty = revoke everything for this tenant).
-- Steps run inside ONE transaction:
--   1. Purge buffered history owned by OTHER tenants for Pages the caller is
--      (re)claiming, so a new owner can never read the previous owner's data.
--   2. Delete other tenants' mappings for those Pages (deterministic takeover
--      even if the previous owner never reconnects).
--   3. Revoke the caller's mappings absent from the verified set.
--   4. Upsert the caller's verified mappings.
CREATE OR REPLACE FUNCTION replace_meta_tenant_pages(p_pages JSONB)
RETURNS INTEGER
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
    v_tenant UUID;
    v_upserted INTEGER;
BEGIN
    v_tenant := auth.uid();
    IF v_tenant IS NULL THEN
        RAISE EXCEPTION 'authentication required';
    END IF;

    DELETE FROM public.meta_webhook_buffer b
     WHERE b.tenant_id <> v_tenant
       AND EXISTS (
            SELECT 1
              FROM jsonb_array_elements(p_pages) AS elem
             WHERE elem->>'page_id' = b.page_id
           );

    DELETE FROM public.meta_tenant_pages p
     WHERE p.tenant_id <> v_tenant
       AND EXISTS (
            SELECT 1
              FROM jsonb_array_elements(p_pages) AS elem
             WHERE elem->>'page_id' = p.page_id
           );

    DELETE FROM public.meta_tenant_pages p
     WHERE p.tenant_id = v_tenant
       AND NOT EXISTS (
            SELECT 1
              FROM jsonb_array_elements(p_pages) AS elem
             WHERE elem->>'page_id' = p.page_id
           );

    INSERT INTO public.meta_tenant_pages (tenant_id, page_id, page_name)
    SELECT v_tenant, elem->>'page_id', COALESCE(elem->>'page_name', '')
      FROM jsonb_array_elements(p_pages) AS elem
     WHERE COALESCE(elem->>'page_id', '') <> ''
    ON CONFLICT (tenant_id, page_id)
    DO UPDATE SET page_name = EXCLUDED.page_name;

    GET DIAGNOSTICS v_upserted = ROW_COUNT;
    RETURN v_upserted;
END;
$$;

REVOKE ALL ON FUNCTION replace_meta_tenant_pages(JSONB) FROM PUBLIC;
REVOKE ALL ON FUNCTION replace_meta_tenant_pages(JSONB) FROM anon;
GRANT EXECUTE ON FUNCTION replace_meta_tenant_pages(JSONB) TO authenticated;

-- Server-controlled status transition: tenants may only flip pending -> synced
-- on rows ingested under their own tenancy. Payload/event identity columns
-- stay immutable because this function updates status only.
CREATE OR REPLACE FUNCTION mark_meta_buffer_synced(p_ids BIGINT[])
RETURNS INTEGER
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
    v_updated INTEGER;
BEGIN
    UPDATE public.meta_webhook_buffer b
       SET status = 'synced'
     WHERE b.tenant_id = auth.uid()
       AND b.status = 'pending'
       AND b.id = ANY(p_ids);
    GET DIAGNOSTICS v_updated = ROW_COUNT;
    RETURN v_updated;
END;
$$;

REVOKE ALL ON FUNCTION mark_meta_buffer_synced(BIGINT[]) FROM PUBLIC;
REVOKE ALL ON FUNCTION mark_meta_buffer_synced(BIGINT[]) FROM anon;
GRANT EXECUTE ON FUNCTION mark_meta_buffer_synced(BIGINT[]) TO authenticated;
