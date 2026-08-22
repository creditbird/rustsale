-- Forward cleanup for environments that applied 20260821090000_meta_relay.sql.
--
-- The superseded relay architecture (guard-mutex RPCs, guard tables, old
-- buffer shape) was merged to master before the revised plan was adopted.
-- This migration removes those superseded objects and, when the legacy
-- meta_webhook_buffer shape exists (event_type column), transforms it to the
-- revised shape required by ingest_meta_webhook_batch:
--   event_type -> field
--   status synced/dead_letter -> completed/failed
-- Safe no-op on fresh databases where the superseded objects never existed.

DROP FUNCTION IF EXISTS public.mark_meta_buffer_synced(BIGINT[]);
DROP FUNCTION IF EXISTS public.stage_meta_webhook_events(JSONB);
DROP FUNCTION IF EXISTS public.replace_meta_tenant_pages(UUID, JSONB);

DROP TABLE IF EXISTS public.meta_page_guards;
DROP TABLE IF EXISTS public.meta_tenant_guards;

DROP POLICY IF EXISTS "Tenants read own page buffer" ON public.meta_webhook_buffer;

DO $$
DECLARE
    v_has_legacy_shape BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1
          FROM information_schema.columns
         WHERE table_schema = 'public'
           AND table_name = 'meta_webhook_buffer'
           AND column_name = 'event_type'
    ) INTO v_has_legacy_shape;

    IF v_has_legacy_shape THEN
        -- Map legacy statuses onto the revised state machine.
        UPDATE public.meta_webhook_buffer SET status = 'completed' WHERE status = 'synced';
        UPDATE public.meta_webhook_buffer SET status = 'failed' WHERE status = 'dead_letter';

        ALTER TABLE public.meta_webhook_buffer RENAME COLUMN event_type TO field;

        -- Replace legacy dedup/index structures with the revised ones.
        DROP INDEX IF EXISTS public.idx_meta_buffer_dedup;
        DROP INDEX IF EXISTS public.idx_meta_buffer_sync;
        CREATE UNIQUE INDEX IF NOT EXISTS idx_meta_buffer_dedup
            ON public.meta_webhook_buffer(tenant_id, page_id, event_key);
        CREATE INDEX IF NOT EXISTS idx_meta_buffer_queue
            ON public.meta_webhook_buffer(tenant_id, status, id);

        -- Enforce the revised field/status constraints on legacy rows.
        DELETE FROM public.meta_webhook_buffer WHERE field IS NULL OR field = '';
        ALTER TABLE public.meta_webhook_buffer
            ALTER COLUMN field SET NOT NULL;
        ALTER TABLE public.meta_webhook_buffer
            ADD CONSTRAINT meta_webhook_buffer_field_check CHECK (field <> '');
        ALTER TABLE public.meta_webhook_buffer
            DROP CONSTRAINT IF EXISTS meta_webhook_buffer_status_check;
        ALTER TABLE public.meta_webhook_buffer
            ADD CONSTRAINT meta_webhook_buffer_status_check
            CHECK (status IN ('pending', 'processing', 'completed', 'failed'));
    END IF;
END
$$;
