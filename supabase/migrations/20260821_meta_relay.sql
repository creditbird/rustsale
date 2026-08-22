-- Facebook/Meta relay schema: tenant-page mapping + durable webhook buffer
--
-- Security model (authoritative):
-- - meta_tenant_pages rows are written ONLY by Edge Functions using the
--   service role key AFTER Meta-side ownership verification (/me/accounts).
-- - Tenants get read-only access to their own mappings and buffer rows via RLS.
-- - Webhook payloads are immutable to clients: there is deliberately NO
--   INSERT/UPDATE/DELETE policy for tenants on meta_webhook_buffer.
--   Status transitions go through mark_meta_buffer_synced() which verifies
--   ownership inside a SECURITY DEFINER boundary.
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
    page_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    event_key TEXT NOT NULL,
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'synced', 'dead_letter')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_meta_buffer_dedup ON meta_webhook_buffer(page_id, event_type, event_key);
CREATE INDEX IF NOT EXISTS idx_meta_buffer_sync ON meta_webhook_buffer(page_id, status, id);

ALTER TABLE meta_webhook_buffer ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Tenants read own page buffer" ON meta_webhook_buffer
    FOR SELECT USING (
        page_id IN (SELECT page_id FROM meta_tenant_pages WHERE tenant_id = auth.uid())
    );

-- Server-controlled status transition: tenants may only flip pending -> synced
-- on rows whose page they provably own. Payload/event identity columns stay
-- immutable because this function updates status only.
CREATE OR REPLACE FUNCTION mark_meta_buffer_synced(p_ids BIGINT[])
RETURNS INTEGER
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
    v_updated INTEGER;
BEGIN
    UPDATE meta_webhook_buffer b
       SET status = 'synced'
      FROM meta_tenant_pages m
     WHERE b.page_id = m.page_id
       AND m.tenant_id = auth.uid()
       AND b.id = ANY(p_ids)
       AND b.status = 'pending';
    GET DIAGNOSTICS v_updated = ROW_COUNT;
    RETURN v_updated;
END;
$$;

REVOKE ALL ON FUNCTION mark_meta_buffer_synced(BIGINT[]) FROM PUBLIC;
REVOKE ALL ON FUNCTION mark_meta_buffer_synced(BIGINT[]) FROM anon;
GRANT EXECUTE ON FUNCTION mark_meta_buffer_synced(BIGINT[]) TO authenticated;
