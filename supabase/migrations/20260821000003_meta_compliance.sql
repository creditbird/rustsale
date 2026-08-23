-- Meta Data Deletion Compliance, Multi-Device Tombstones, and Task Distribution (Phase 3)

CREATE TABLE IF NOT EXISTS tenant_registered_devices (
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    device_secret_hash TEXT NOT NULL,
    device_name TEXT,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(tenant_id, device_id)
);
ALTER TABLE tenant_registered_devices ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE tenant_registered_devices FROM PUBLIC, anon, authenticated;
GRANT SELECT (tenant_id, device_id, device_name, last_seen_at) ON TABLE tenant_registered_devices TO authenticated;
GRANT ALL ON TABLE tenant_registered_devices TO service_role;

CREATE POLICY "Tenants manage own devices" ON tenant_registered_devices
    FOR SELECT TO authenticated USING (auth.uid() = tenant_id);

CREATE TABLE IF NOT EXISTS meta_data_deletion_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    confirmation_code TEXT NOT NULL UNIQUE,
    user_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
ALTER TABLE meta_data_deletion_requests ENABLE ROW LEVEL SECURITY;

CREATE TABLE IF NOT EXISTS meta_tenant_deletion_tombstones (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL REFERENCES meta_data_deletion_requests(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    page_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(tenant_id, page_id, user_id)
);
ALTER TABLE meta_tenant_deletion_tombstones ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE meta_tenant_deletion_tombstones FROM PUBLIC, anon, authenticated;
GRANT SELECT ON TABLE meta_tenant_deletion_tombstones TO authenticated;
GRANT ALL ON TABLE meta_tenant_deletion_tombstones TO service_role;

CREATE POLICY "Tenants read own deletion tombstones" ON meta_tenant_deletion_tombstones
    FOR SELECT TO authenticated USING (auth.uid() = tenant_id);

CREATE TABLE IF NOT EXISTS meta_device_deletion_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL REFERENCES meta_data_deletion_requests(id) ON DELETE CASCADE,
    tenant_id UUID NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    page_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'acknowledged', 'completed', 'failed')),
    lease_token_hash TEXT,
    acknowledged_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_meta_device_deletion_task UNIQUE (request_id, tenant_id, device_id, page_id, user_id)
);
ALTER TABLE meta_device_deletion_tasks ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE meta_device_deletion_tasks FROM PUBLIC, anon, authenticated;
GRANT SELECT (id, request_id, tenant_id, device_id, page_id, user_id, status, acknowledged_at, completed_at, created_at, updated_at) ON TABLE meta_device_deletion_tasks TO authenticated;
GRANT ALL ON TABLE meta_device_deletion_tasks TO service_role;

CREATE POLICY "Tenants read own device deletion tasks" ON meta_device_deletion_tasks
    FOR SELECT TO authenticated USING (auth.uid() = tenant_id);

CREATE OR REPLACE FUNCTION claim_device_deletion_tasks(
    p_device_id TEXT,
    p_device_secret TEXT,
    p_lease_seconds INT DEFAULT 300
)
RETURNS TABLE (
    id UUID,
    request_id UUID,
    page_id TEXT,
    user_id TEXT,
    lease_token TEXT,
    created_at TIMESTAMPTZ
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, auth, extensions, pg_temp
AS $$
DECLARE
    v_raw_token TEXT;
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM public.tenant_registered_devices
        WHERE tenant_id = auth.uid()
          AND device_id = p_device_id
          AND device_secret_hash = encode(digest(p_device_secret, 'sha256'), 'hex')
    ) THEN
        RAISE EXCEPTION 'Unauthorized: Invalid device credentials';
    END IF;

    v_raw_token := encode(gen_random_bytes(32), 'hex');

    RETURN QUERY
    UPDATE public.meta_device_deletion_tasks
    SET status = 'acknowledged',
        lease_token_hash = encode(digest(v_raw_token, 'sha256'), 'hex'),
        acknowledged_at = now(),
        updated_at = now()
    WHERE meta_device_deletion_tasks.id IN (
        SELECT t.id FROM public.meta_device_deletion_tasks t
        WHERE t.tenant_id = auth.uid()
          AND t.device_id = p_device_id
          AND (
              t.status = 'pending'
              OR (t.status = 'acknowledged' AND t.acknowledged_at < now() - (p_lease_seconds || ' seconds')::INTERVAL)
          )
        ORDER BY t.created_at ASC
        LIMIT 10
        FOR UPDATE SKIP LOCKED
    )
    RETURNING meta_device_deletion_tasks.id, meta_device_deletion_tasks.request_id, meta_device_deletion_tasks.page_id, meta_device_deletion_tasks.user_id, v_raw_token AS lease_token, meta_device_deletion_tasks.created_at;
END;
$$;

CREATE OR REPLACE FUNCTION complete_device_deletion_task(
    p_task_id UUID,
    p_device_id TEXT,
    p_device_secret TEXT,
    p_lease_token TEXT
)
RETURNS BOOLEAN
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, auth, extensions, pg_temp
AS $$
DECLARE
    v_updated INT;
    v_request_id UUID;
    v_req_status TEXT;
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM public.tenant_registered_devices
        WHERE tenant_id = auth.uid()
          AND device_id = p_device_id
          AND device_secret_hash = encode(digest(p_device_secret, 'sha256'), 'hex')
    ) THEN
        RAISE EXCEPTION 'Unauthorized: Invalid device credentials';
    END IF;

    UPDATE public.meta_device_deletion_tasks
    SET status = 'completed',
        completed_at = now(),
        lease_token_hash = NULL,
        updated_at = now()
    WHERE id = p_task_id
      AND tenant_id = auth.uid()
      AND device_id = p_device_id
      AND lease_token_hash = encode(digest(p_lease_token, 'sha256'), 'hex')
      AND status = 'acknowledged'
    RETURNING request_id INTO v_request_id;

    GET DIAGNOSTICS v_updated = ROW_COUNT;

    -- If all deletion tasks for this request_id across all tenants/devices are completed, update request status to completed with serialized row lock
    IF v_updated > 0 AND v_request_id IS NOT NULL THEN
        SELECT status INTO v_req_status
        FROM public.meta_data_deletion_requests
        WHERE id = v_request_id
        FOR UPDATE;

        IF NOT EXISTS (
            SELECT 1 FROM public.meta_device_deletion_tasks
            WHERE request_id = v_request_id AND status != 'completed'
        ) THEN
            UPDATE public.meta_data_deletion_requests
            SET status = 'completed', completed_at = now(), updated_at = now()
            WHERE id = v_request_id;
        END IF;
    END IF;

    RETURN v_updated > 0;
END;
$$;

CREATE OR REPLACE FUNCTION release_device_deletion_task(
    p_task_id UUID,
    p_device_id TEXT,
    p_device_secret TEXT,
    p_lease_token TEXT
)
RETURNS BOOLEAN
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, auth, extensions, pg_temp
AS $$
DECLARE
    v_updated INT;
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM public.tenant_registered_devices
        WHERE tenant_id = auth.uid()
          AND device_id = p_device_id
          AND device_secret_hash = encode(digest(p_device_secret, 'sha256'), 'hex')
    ) THEN
        RAISE EXCEPTION 'Unauthorized: Invalid device credentials';
    END IF;

    UPDATE public.meta_device_deletion_tasks
    SET status = 'pending',
        lease_token_hash = NULL,
        acknowledged_at = NULL,
        updated_at = now()
    WHERE id = p_task_id
      AND tenant_id = auth.uid()
      AND device_id = p_device_id
      AND lease_token_hash = encode(digest(p_lease_token, 'sha256'), 'hex')
      AND status = 'acknowledged';

    GET DIAGNOSTICS v_updated = ROW_COUNT;
    RETURN v_updated > 0;
END;
$$;

CREATE OR REPLACE FUNCTION register_or_heartbeat_device(
    p_device_id TEXT,
    p_device_secret TEXT,
    p_device_name TEXT DEFAULT NULL
)
RETURNS BOOLEAN
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, auth, extensions, pg_temp
AS $$
DECLARE
    v_secret_hash TEXT;
    v_existing_hash TEXT;
BEGIN
    IF p_device_secret IS NULL OR length(p_device_secret) < 32 THEN
        RAISE EXCEPTION 'Invalid device secret: minimum 32 characters required';
    END IF;

    v_secret_hash := encode(digest(p_device_secret, 'sha256'), 'hex');

    SELECT device_secret_hash INTO v_existing_hash
    FROM public.tenant_registered_devices
    WHERE tenant_id = auth.uid() AND device_id = p_device_id;

    IF v_existing_hash IS NOT NULL THEN
        IF v_existing_hash != v_secret_hash THEN
            RAISE EXCEPTION 'Unauthorized: Device credential mismatch';
        END IF;
        UPDATE public.tenant_registered_devices
        SET last_seen_at = now(),
            device_name = COALESCE(p_device_name, device_name)
        WHERE tenant_id = auth.uid() AND device_id = p_device_id;
    ELSE
        INSERT INTO public.tenant_registered_devices (tenant_id, device_id, device_secret_hash, device_name, last_seen_at)
        VALUES (auth.uid(), p_device_id, v_secret_hash, p_device_name, now());
    END IF;

    -- Serialize against concurrent deletion completion by locking active deletion requests relevant to this tenant
    PERFORM 1
    FROM public.meta_data_deletion_requests r
    WHERE r.status = 'processing'
      AND EXISTS (
          SELECT 1 FROM public.meta_tenant_deletion_tombstones t
          WHERE t.request_id = r.id AND t.tenant_id = auth.uid()
      )
    FOR UPDATE;

    -- Durable scrub-on-reconnect: Backfill pending deletion tasks for any tenant deletion tombstones where this device does not already have an active/completed task for that specific request
    INSERT INTO public.meta_device_deletion_tasks (request_id, tenant_id, device_id, page_id, user_id, status)
    SELECT t.request_id, t.tenant_id, p_device_id, t.page_id, t.user_id, 'pending'
    FROM public.meta_tenant_deletion_tombstones t
    WHERE t.tenant_id = auth.uid()
    ON CONFLICT (request_id, tenant_id, device_id, page_id, user_id) DO NOTHING;

    -- Reopen request if new pending tasks were created for a request that was previously marked completed
    UPDATE public.meta_data_deletion_requests r
    SET status = 'processing', completed_at = NULL, updated_at = now()
    WHERE r.id IN (
        SELECT dt.request_id
        FROM public.meta_device_deletion_tasks dt
        WHERE dt.tenant_id = auth.uid()
          AND dt.device_id = p_device_id
          AND dt.status = 'pending'
    ) AND r.status = 'completed';

    RETURN TRUE;
END;
$$;

REVOKE ALL ON FUNCTION register_or_heartbeat_device, claim_device_deletion_tasks, complete_device_deletion_task, release_device_deletion_task FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION register_or_heartbeat_device, claim_device_deletion_tasks, complete_device_deletion_task, release_device_deletion_task TO authenticated, service_role;

CREATE OR REPLACE FUNCTION create_data_deletion_request_and_tasks(
    p_confirmation_code TEXT,
    p_user_id TEXT,
    p_target_mappings JSONB DEFAULT '[]'::JSONB
)
RETURNS UUID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, auth, pg_temp
AS $$
DECLARE
    v_request_id UUID;
    v_tenant_record RECORD;
    v_device_record RECORD;
    v_has_active_devices BOOLEAN := FALSE;
BEGIN
    INSERT INTO public.meta_data_deletion_requests (confirmation_code, user_id, status)
    VALUES (p_confirmation_code, p_user_id, 'processing')
    RETURNING id INTO v_request_id;

    -- Collect all distinct target (tenant_id, page_id, page_scoped_user_id) tuples matching either authoritative root app_user_id or explicit resolved (page_id, psid) mapping
    FOR v_tenant_record IN
        SELECT DISTINCT mupi.tenant_id, mupi.page_id, mupi.page_scoped_user_id
        FROM public.meta_user_page_interactions mupi
        WHERE mupi.app_user_id = p_user_id
           OR EXISTS (
               SELECT 1 FROM jsonb_array_elements(p_target_mappings) m
               WHERE (m->>'page_id' IS NOT NULL AND m->>'page_id' <> '')
                 AND mupi.page_id = (m->>'page_id')
                 AND mupi.page_scoped_user_id = (m->>'psid')
           )
    LOOP
        -- Persist durable tenant-scoped deletion tombstone with authoritative request_id
        INSERT INTO public.meta_tenant_deletion_tombstones (request_id, tenant_id, page_id, user_id)
        VALUES (v_request_id, v_tenant_record.tenant_id, v_tenant_record.page_id, v_tenant_record.page_scoped_user_id)
        ON CONFLICT (tenant_id, page_id, user_id)
        DO UPDATE SET request_id = EXCLUDED.request_id, created_at = now();

        -- Purge server-side buffered webhooks strictly scoped to this tenant_id and page_id
        DELETE FROM public.meta_webhook_buffer
        WHERE tenant_id = v_tenant_record.tenant_id
          AND (page_id = v_tenant_record.page_id OR page_id IS NULL)
          AND (
              payload->'sender'->>'id' = v_tenant_record.page_scoped_user_id
              OR payload->'recipient'->>'id' = v_tenant_record.page_scoped_user_id
              OR payload->'from'->>'id' = v_tenant_record.page_scoped_user_id
              OR payload->'value'->'from'->>'id' = v_tenant_record.page_scoped_user_id
              OR payload->'value'->'sender'->>'id' = v_tenant_record.page_scoped_user_id
              OR payload->'value'->>'sender_id' = v_tenant_record.page_scoped_user_id
              OR payload->'value'->>'user_id' = v_tenant_record.page_scoped_user_id
          );

        -- Fan out tasks to all registered tenant devices regardless of offline duration
        FOR v_device_record IN
            SELECT device_id FROM public.tenant_registered_devices
            WHERE tenant_id = v_tenant_record.tenant_id
        LOOP
            v_has_active_devices := TRUE;
            -- Fan out deletion task strictly carrying page_id and page-scoped user_id to be scrubbed on desktop SQLite
            INSERT INTO public.meta_device_deletion_tasks (request_id, tenant_id, device_id, page_id, user_id, status)
            VALUES (v_request_id, v_tenant_record.tenant_id, v_device_record.device_id, v_tenant_record.page_id, v_tenant_record.page_scoped_user_id, 'pending')
            ON CONFLICT (request_id, tenant_id, device_id, page_id, user_id) DO NOTHING;
        END LOOP;

        -- Clean up interaction records strictly scoped to this tenant, page, and user
        DELETE FROM public.meta_user_page_interactions
        WHERE tenant_id = v_tenant_record.tenant_id
          AND page_id = v_tenant_record.page_id
          AND (app_user_id = p_user_id OR page_scoped_user_id = v_tenant_record.page_scoped_user_id);
    END LOOP;

    -- Clean up any residual global interaction records matching authoritative app_user_id
    DELETE FROM public.meta_user_page_interactions
    WHERE app_user_id = p_user_id;

    -- If no registered devices exist for target tenants and no tombstones were created, mark request completed immediately
    IF NOT v_has_active_devices AND NOT EXISTS (SELECT 1 FROM public.meta_tenant_deletion_tombstones WHERE request_id = v_request_id) THEN
        UPDATE public.meta_data_deletion_requests SET status = 'completed', completed_at = now(), updated_at = now() WHERE id = v_request_id;
    END IF;

    RETURN v_request_id;
END;
$$;

REVOKE ALL ON FUNCTION create_data_deletion_request_and_tasks FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION create_data_deletion_request_and_tasks TO service_role;
