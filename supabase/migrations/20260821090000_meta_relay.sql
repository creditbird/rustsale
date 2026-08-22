-- Facebook/Meta relay schema: tenant-page mapping + durable webhook buffer
--
-- Security model (authoritative):
-- - Tenants hold SELECT-only RLS on both tables. No client-writable surface.
-- - Page ownership reconciliation happens ONLY through
--   replace_meta_tenant_pages(): SERVICE-ROLE ONLY (EXECUTE revoked from
--   public/anon/authenticated), called exclusively by /meta-oauth with the
--   tenant id taken from the caller's verified Supabase JWT. It is atomic:
--   takeover purge -> takeover delete -> revocation -> upsert all-or-nothing.
--   Ownership rule (deliberate product decision): the LATEST VERIFIED PAGE
--   ADMINISTRATOR WINS. Because Meta can authorize several admins for the
--   same Page, successful /me/accounts enumeration proves manage rights but
--   not exclusivity; connecting evicts any previous tenant's mapping AND
--   purges that tenant's buffered history for the claimed Pages (privacy
--   retention rule: relay data belongs to the ownership relationship).
-- - Webhook ingestion happens ONLY through stage_meta_webhook_events():
--   SERVICE-ROLE ONLY. It resolves each Page's current owner and inserts
--   buffer rows inside ONE transaction while holding FOR SHARE locks on the
--   stable meta_page_guards rows, so ingestion and ownership replacement are
--   serialized per
--   Page: an event can never be persisted under the previous tenant AFTER a
--   takeover has committed, and events persisted just BEFORE a takeover are
--   purged by it under the documented retention rule.
-- - meta_webhook_buffer.tenant_id is stamped at ingestion time and never
--   updated: authorization is time-stable across later ownership changes.
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

-- Stable per-Page mutex rows. Mapping tuples are deleted and recreated by
-- takeover, so they cannot serve as reliable lock targets across concurrent
-- reconciliations. Guard rows are keyed by page_id and NEVER deleted: both
-- privileged RPCs take transaction-scoped row locks on them (FOR UPDATE for
-- replacement, FOR SHARE for ingestion), giving a churn-proof serialization
-- point per Page. Accessed exclusively inside SECURITY DEFINER functions.
CREATE TABLE IF NOT EXISTS meta_page_guards (
    page_id TEXT PRIMARY KEY
);
REVOKE ALL ON meta_page_guards FROM PUBLIC;
REVOKE ALL ON meta_page_guards FROM anon;
REVOKE ALL ON meta_page_guards FROM authenticated;

-- Stable per-TENANT mutex rows (same churn-proof pattern as page guards):
-- full-set reconciliation is a replacement, so two concurrent replacements
-- for the same tenant must serialize instead of merging into an unverified
-- union state. Never deleted; touched only inside SECURITY DEFINER functions.
CREATE TABLE IF NOT EXISTS meta_tenant_guards (
    tenant_id UUID PRIMARY KEY
);
REVOKE ALL ON meta_tenant_guards FROM PUBLIC;
REVOKE ALL ON meta_tenant_guards FROM anon;
REVOKE ALL ON meta_tenant_guards FROM authenticated;

-- No INSERT/UPDATE/DELETE policies exist for clients: payloads stay immutable
-- and status transitions go exclusively through the RPC below.

CREATE OR REPLACE FUNCTION replace_meta_tenant_pages(
    p_tenant_id UUID,
    p_pages JSONB
)
RETURNS INTEGER
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
    v_upserted INTEGER;
BEGIN
    -- Defensive input validation (server-only callers today, but make
    -- failures deterministic anyway).
    IF p_tenant_id IS NULL THEN
        RAISE EXCEPTION 'tenant id required';
    END IF;
    IF jsonb_typeof(p_pages) <> 'array' THEN
        RAISE EXCEPTION 'p_pages must be a JSON array';
    END IF;

    -- Tenant-level mutex first: full-set replacement must serialize against
    -- concurrent replacements for the SAME tenant (two devices / repeated
    -- OAuth callbacks), otherwise the result could be an unverified union of
    -- two enumerations.
    INSERT INTO public.meta_tenant_guards (tenant_id)
    VALUES (p_tenant_id)
    ON CONFLICT (tenant_id) DO NOTHING;
    PERFORM 1
      FROM public.meta_tenant_guards g
     WHERE g.tenant_id = p_tenant_id
       FOR UPDATE OF g;

    -- Page-level serialization on a STABLE lock target. The lock set is the
    -- UNION of (a) the freshly verified Pages and (b) every Page currently
    -- mapped to this tenant — revoked-by-omission Pages must participate in
    -- the same mutex, otherwise a webhook could persist events under this
    -- tenant after revocation committed.
    CREATE TEMP TABLE _replace_page_ids (page_id TEXT PRIMARY KEY) ON COMMIT DROP;
    INSERT INTO _replace_page_ids (page_id)
    SELECT DISTINCT elem->>'page_id'
      FROM jsonb_array_elements(p_pages) AS elem
     WHERE COALESCE(elem->>'page_id', '') <> '';

    CREATE TEMP TABLE _lock_page_ids (page_id TEXT PRIMARY KEY) ON COMMIT DROP;
    INSERT INTO _lock_page_ids (page_id)
        SELECT page_id FROM _replace_page_ids
    UNION
        SELECT m.page_id
          FROM public.meta_tenant_pages m
         WHERE m.tenant_id = p_tenant_id;

    INSERT INTO public.meta_page_guards (page_id)
    SELECT page_id FROM _lock_page_ids
     ORDER BY page_id
    ON CONFLICT (page_id) DO NOTHING;

    PERFORM 1
      FROM public.meta_page_guards g
      JOIN _lock_page_ids s ON s.page_id = g.page_id
     ORDER BY g.page_id
       FOR UPDATE OF g;

    -- 1. Purge buffered history owned by OTHER tenants for Pages the caller
    --    is (re)claiming (documented retention-on-transfer rule).
    DELETE FROM public.meta_webhook_buffer b
     WHERE b.tenant_id <> p_tenant_id
       AND EXISTS (
            SELECT 1
              FROM jsonb_array_elements(p_pages) AS elem
             WHERE elem->>'page_id' = b.page_id
           );

    -- 2. Deterministic takeover: remove other tenants' mappings for the
    --    freshly verified Pages.
    DELETE FROM public.meta_tenant_pages p
     WHERE p.tenant_id <> p_tenant_id
       AND EXISTS (
            SELECT 1
              FROM jsonb_array_elements(p_pages) AS elem
             WHERE elem->>'page_id' = p.page_id
           );

    -- 3. Revoke the caller's mappings absent from the verified set
    --    (zero-Page enumeration revokes everything).
    DELETE FROM public.meta_tenant_pages p
     WHERE p.tenant_id = p_tenant_id
       AND NOT EXISTS (
            SELECT 1
              FROM jsonb_array_elements(p_pages) AS elem
             WHERE elem->>'page_id' = p.page_id
           );

    -- 4. Upsert the caller's verified mappings.
    INSERT INTO public.meta_tenant_pages (tenant_id, page_id, page_name)
    SELECT p_tenant_id, elem->>'page_id', COALESCE(elem->>'page_name', '')
      FROM jsonb_array_elements(p_pages) AS elem
     WHERE COALESCE(elem->>'page_id', '') <> ''
    ON CONFLICT (tenant_id, page_id)
    DO UPDATE SET page_name = EXCLUDED.page_name;

    GET DIAGNOSTICS v_upserted = ROW_COUNT;
    RETURN v_upserted;
END;
$$;

REVOKE ALL ON FUNCTION replace_meta_tenant_pages(UUID, JSONB) FROM PUBLIC;
REVOKE ALL ON FUNCTION replace_meta_tenant_pages(UUID, JSONB) FROM anon;
REVOKE ALL ON FUNCTION replace_meta_tenant_pages(UUID, JSONB) FROM authenticated;
GRANT EXECUTE ON FUNCTION replace_meta_tenant_pages(UUID, JSONB) TO service_role;

-- Atomic webhook ingestion: resolve current owner per Page and persist rows
-- in one transaction while holding FOR SHARE locks on the stable guard rows, so a
-- concurrent replace_meta_tenant_pages() cannot interleave between snapshot
-- and insert (and vice versa). Events for unmapped Pages are counted and NOT
-- stored: without a durable owning tenant they could never be authorized.
CREATE OR REPLACE FUNCTION stage_meta_webhook_events(p_rows JSONB)
RETURNS JSONB
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
    v_inserted INTEGER := 0;
    v_dropped_unmapped INTEGER := 0;
BEGIN
    IF jsonb_typeof(p_rows) <> 'array' THEN
        RAISE EXCEPTION 'p_rows must be a JSON array';
    END IF;

    -- Serialize against concurrent reconciliation on the STABLE guard rows:
    -- FOR SHARE conflicts with the FOR UPDATE taken by
    -- replace_meta_tenant_pages(), so ownership replacement and ingestion
    -- fully serialize per Page regardless of mapping-row churn.
    CREATE TEMP TABLE _stage_page_ids (page_id TEXT PRIMARY KEY) ON COMMIT DROP;

    INSERT INTO _stage_page_ids (page_id)
    SELECT DISTINCT elem->>'page_id'
      FROM jsonb_array_elements(p_rows) AS elem
     WHERE COALESCE(elem->>'page_id', '') <> '';

    INSERT INTO public.meta_page_guards (page_id)
    SELECT page_id FROM _stage_page_ids
     ORDER BY page_id
    ON CONFLICT (page_id) DO NOTHING;

    PERFORM 1
      FROM public.meta_page_guards g
      JOIN _stage_page_ids s ON s.page_id = g.page_id
     ORDER BY g.page_id
       FOR SHARE OF g;

    CREATE TEMP TABLE _stage_buffer_rows (
        page_id TEXT NOT NULL,
        event_type TEXT NOT NULL,
        event_key TEXT NOT NULL,
        payload JSONB NOT NULL
    ) ON COMMIT DROP;

    INSERT INTO _stage_buffer_rows (page_id, event_type, event_key, payload)
    SELECT elem->>'page_id',
           elem->>'event_type',
           elem->>'event_key',
           elem->'payload'
      FROM jsonb_array_elements(p_rows) AS elem
     WHERE COALESCE(elem->>'page_id', '') <> ''
       AND COALESCE(elem->>'event_type', '') <> ''
       AND COALESCE(elem->>'event_key', '') <> ''
       AND jsonb_typeof(elem->'payload') = 'object';

    WITH resolved AS (
        SELECT s.page_id,
               s.event_type,
               s.event_key,
               s.payload,
               m.tenant_id
          FROM _stage_buffer_rows s
          LEFT JOIN public.meta_tenant_pages m ON m.page_id = s.page_id
    ),
    inserted AS (
        INSERT INTO public.meta_webhook_buffer
            (tenant_id, page_id, event_type, event_key, payload, status)
        SELECT tenant_id, page_id, event_type, event_key, payload, 'pending'
          FROM resolved
         WHERE tenant_id IS NOT NULL
        ON CONFLICT (page_id, event_type, event_key)
        DO NOTHING
        RETURNING 1 AS ok
    )
    SELECT (SELECT count(*)::int FROM inserted) AS accepted,
           (SELECT count(*)::int FROM resolved WHERE tenant_id IS NULL) AS dropped_unmapped
    INTO v_inserted, v_dropped_unmapped;

    RETURN jsonb_build_object('accepted', v_inserted, 'dropped_unmapped', v_dropped_unmapped);
END;
$$;

REVOKE ALL ON FUNCTION stage_meta_webhook_events(JSONB) FROM PUBLIC;
REVOKE ALL ON FUNCTION stage_meta_webhook_events(JSONB) FROM anon;
REVOKE ALL ON FUNCTION stage_meta_webhook_events(JSONB) FROM authenticated;
GRANT EXECUTE ON FUNCTION stage_meta_webhook_events(JSONB) TO service_role;

-- Tenant-facing status transition: flips pending -> synced for rows ingested
-- under the CALLER'S OWN tenancy (auth.uid() here is correct because this RPC
-- is executed by desktop clients with their user JWT via PostgREST).
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
