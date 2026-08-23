-- Meta Data Deletion Compliance, Multi-Device Tombstones, Task Distribution, and Immutable Audit Logging (Phase 3)

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
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'partially_completed', 'completed', 'failed')),
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

CREATE TABLE IF NOT EXISTS meta_compliance_audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID REFERENCES meta_data_deletion_requests(id) ON DELETE SET NULL,
    tenant_id UUID REFERENCES auth.users(id) ON DELETE SET NULL,
    device_id TEXT,
    action TEXT NOT NULL,
    details JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
ALTER TABLE meta_compliance_audit_logs ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE meta_compliance_audit_logs FROM PUBLIC, anon, authenticated;
GRANT SELECT ON TABLE meta_compliance_audit_logs TO authenticated;
GRANT ALL ON TABLE meta_compliance_audit_logs TO service_role;

CREATE POLICY "Tenants read own compliance audit logs" ON meta_compliance_audit_logs
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

    UPDATE public.tenant_registered_devices
    SET last_seen_at = now()
    WHERE tenant_id = auth.uid() AND device_id = p_device_id;

    v_raw_token := encode(gen_random_bytes(32), 'hex');

    RETURN QUERY
    WITH candidate_tasks AS (
        SELECT t.id AS target_id
        FROM public.meta_device_deletion_tasks t
        WHERE t.tenant_id = auth.uid()
          AND t.device_id = p_device_id
          AND (
              t.status = 'pending'
              OR (
                  t.status = 'acknowledged'
                  AND t.acknowledged_at < now() - (COALESCE(p_lease_seconds, 300) || ' seconds')::INTERVAL
              )
          )
        ORDER BY t.created_at ASC
        LIMIT 10
        FOR UPDATE SKIP LOCKED
    ),
    updated_tasks AS (
        UPDATE public.meta_device_deletion_tasks t
        SET status = 'acknowledged',
            lease_token_hash = encode(digest(v_raw_token, 'sha256'), 'hex'),
            acknowledged_at = now(),
            updated_at = now()
        FROM candidate_tasks c
        WHERE t.id = c.target_id
        RETURNING t.id, t.request_id, t.page_id, t.user_id, t.created_at
    )
    SELECT u.id, u.request_id, u.page_id, u.user_id, v_raw_token, u.created_at
    FROM updated_tasks u;
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
    v_task RECORD;
    v_req_id UUID;
    v_uncompleted_count INT;
    v_total_tasks INT;
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
      AND status = 'acknowledged'
      AND lease_token_hash = encode(digest(p_lease_token, 'sha256'), 'hex')
    RETURNING request_id INTO v_req_id;

    IF v_req_id IS NULL THEN
        RETURN FALSE;
    END IF;

    -- Audit log
    INSERT INTO public.meta_compliance_audit_logs (request_id, tenant_id, device_id, action, details)
    VALUES (v_req_id, auth.uid(), p_device_id, 'device_task_completed', jsonb_build_object('task_id', p_task_id));

    -- Check overall completion of all distributed device tasks for this parent request
    SELECT COUNT(*), COUNT(*) FILTER (WHERE status != 'completed')
    INTO v_total_tasks, v_uncompleted_count
    FROM public.meta_device_deletion_tasks
    WHERE request_id = v_req_id;

    IF v_uncompleted_count = 0 THEN
        UPDATE public.meta_data_deletion_requests
        SET status = 'completed',
            completed_at = now(),
            updated_at = now()
        WHERE id = v_req_id;

        INSERT INTO public.meta_compliance_audit_logs (request_id, tenant_id, device_id, action, details)
        VALUES (v_req_id, auth.uid(), p_device_id, 'request_fully_completed', jsonb_build_object('total_tasks', v_total_tasks));
    ELSE
        UPDATE public.meta_data_deletion_requests
        SET status = 'partially_completed',
            updated_at = now()
        WHERE id = v_req_id AND status != 'completed';
    END IF;

    RETURN TRUE;
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
    v_req_id UUID;
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
      AND status = 'acknowledged'
      AND lease_token_hash = encode(digest(p_lease_token, 'sha256'), 'hex')
    RETURNING request_id INTO v_req_id;

    IF v_req_id IS NOT NULL THEN
        INSERT INTO public.meta_compliance_audit_logs (request_id, tenant_id, device_id, action, details)
        VALUES (v_req_id, auth.uid(), p_device_id, 'device_task_released', jsonb_build_object('task_id', p_task_id));
        RETURN TRUE;
    END IF;

    RETURN FALSE;
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
BEGIN
    v_secret_hash := encode(digest(p_device_secret, 'sha256'), 'hex');

    INSERT INTO public.tenant_registered_devices (tenant_id, device_id, device_secret_hash, device_name, last_seen_at)
    VALUES (auth.uid(), p_device_id, v_secret_hash, p_device_name, now())
    ON CONFLICT (tenant_id, device_id) DO UPDATE
    SET last_seen_at = now(),
        device_name = COALESCE(EXCLUDED.device_name, public.tenant_registered_devices.device_name)
    WHERE public.tenant_registered_devices.device_secret_hash = v_secret_hash;

    -- Durable reconnect tombstone scrub check
    INSERT INTO public.meta_device_deletion_tasks (request_id, tenant_id, device_id, page_id, user_id, status, created_at, updated_at)
    SELECT t.request_id, t.tenant_id, p_device_id, t.page_id, t.user_id, 'pending', now(), now()
    FROM public.meta_tenant_deletion_tombstones t
    WHERE t.tenant_id = auth.uid()
    ON CONFLICT (request_id, tenant_id, device_id, page_id, user_id) DO NOTHING;

    RETURN TRUE;
END;
$$;

CREATE OR REPLACE FUNCTION create_data_deletion_request_and_tasks(
    p_confirmation_code TEXT,
    p_meta_user_id TEXT,
    p_affected_pages JSONB
)
RETURNS UUID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, auth, extensions, pg_temp
AS $$
DECLARE
    v_request_id UUID;
    v_item RECORD;
BEGIN
    INSERT INTO public.meta_data_deletion_requests (confirmation_code, user_id, status, created_at, updated_at)
    VALUES (p_confirmation_code, p_meta_user_id, 'processing', now(), now())
    RETURNING id INTO v_request_id;

    INSERT INTO public.meta_compliance_audit_logs (request_id, action, details)
    VALUES (v_request_id, 'request_created', jsonb_build_object('user_id', p_meta_user_id, 'affected_pages', p_affected_pages));

    FOR v_item IN
        SELECT 
            (elem->>'tenant_id')::UUID AS tenant_id,
            elem->>'page_id' AS page_id,
            elem->>'psid' AS psid
        FROM jsonb_array_elements(p_affected_pages) AS elem
    LOOP
        INSERT INTO public.meta_tenant_deletion_tombstones (request_id, tenant_id, page_id, user_id, created_at)
        VALUES (v_request_id, v_item.tenant_id, v_item.page_id, v_item.psid, now())
        ON CONFLICT (tenant_id, page_id, user_id) DO NOTHING;

        INSERT INTO public.meta_device_deletion_tasks (request_id, tenant_id, device_id, page_id, user_id, status, created_at, updated_at)
        SELECT v_request_id, v_item.tenant_id, d.device_id, v_item.page_id, v_item.psid, 'pending', now(), now()
        FROM public.tenant_registered_devices d
        WHERE d.tenant_id = v_item.tenant_id
        ON CONFLICT (request_id, tenant_id, device_id, page_id, user_id) DO NOTHING;
    END LOOP;

    RETURN v_request_id;
END;
$$;
