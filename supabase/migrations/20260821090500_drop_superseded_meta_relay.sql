-- Forward cleanup for environments that applied 20260821090000_meta_relay.sql.
--
-- The superseded relay architecture (guard-mutex RPCs, guard tables) was
-- merged to master before the revised plan was adopted. This migration:
--   1. drops the superseded functions/guard tables/policies;
--   2. when a LEGACY-shaped meta_webhook_buffer exists (event_type column),
--      transforms it to the revised shape required by ingest_meta_webhook_batch:
--        event_type -> field (normalized to the canonical webhook namespace)
--        status synced/dead_letter -> completed/failed
--      in an order that cannot violate intermediate constraints
--      (drop old status CHECK -> map values -> add revised constraints),
--      deduplicating tenant-scoped keys before creating the new unique index.
-- Safe no-op on fresh databases where the superseded objects never existed.

DROP FUNCTION IF EXISTS public.mark_meta_buffer_synced(BIGINT[]);
DROP FUNCTION IF EXISTS public.stage_meta_webhook_events(JSONB);
DROP FUNCTION IF EXISTS public.replace_meta_tenant_pages(UUID, JSONB);

DROP TABLE IF EXISTS public.meta_page_guards;
DROP TABLE IF EXISTS public.meta_tenant_guards;

-- Legacy policy names from both revisions of 090000.
DROP POLICY IF EXISTS "Tenants read own page buffer" ON public.meta_webhook_buffer;
DROP POLICY IF EXISTS "Tenants read own buffer rows" ON public.meta_webhook_buffer;

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
        -- 1. Remove the legacy status constraint BEFORE remapping values,
        --    otherwise mapping synced->completed violates it mid-flight.
        ALTER TABLE public.meta_webhook_buffer
            DROP CONSTRAINT IF EXISTS meta_webhook_buffer_status_check;

        -- 2. Map legacy statuses onto the revised state machine.
        UPDATE public.meta_webhook_buffer SET status = 'completed' WHERE status = 'synced';
        UPDATE public.meta_webhook_buffer SET status = 'failed' WHERE status = 'dead_letter';

        -- 3. Normalize the field namespace. Legacy messaging-family events
        --    (echoes/deliveries/reads/reactions/messages) all live under the
        --    canonical webhook field "messages"; feed and leadgen keep their
        --    own canonical fields.
        UPDATE public.meta_webhook_buffer
           SET field = 'messages'
         WHERE field IN ('message_echoes', 'message_deliveries', 'message_reads', 'message_reactions', 'messages');

        -- 4. Deduplicate tenant-scoped keys BEFORE creating the unique index:
        --    the legacy unique domain was (page_id, event_type, event_key), so
        --    distinct event_types could share an event_key within one page and
        --    collide once event_type collapses into field. Keep MIN(id).
        DELETE FROM public.meta_webhook_buffer b
         USING public.meta_webhook_buffer b2
         WHERE b.tenant_id = b2.tenant_id
           AND b.page_id = b2.page_id
           AND b.event_key = b2.event_key
           AND b.id > b2.id;

        -- 5. Replace legacy index structures with the revised ones.
        DROP INDEX IF EXISTS public.idx_meta_buffer_dedup;
        DROP INDEX IF EXISTS public.idx_meta_buffer_sync;
        CREATE UNIQUE INDEX IF NOT EXISTS idx_meta_buffer_dedup
            ON public.meta_webhook_buffer(tenant_id, page_id, event_key);
        CREATE INDEX IF NOT EXISTS idx_meta_buffer_queue
            ON public.meta_webhook_buffer(tenant_id, status, id);

        -- 6. Enforce revised column constraints on the transformed table.
        DELETE FROM public.meta_webhook_buffer WHERE field IS NULL OR field = '';
        ALTER TABLE public.meta_webhook_buffer ALTER COLUMN field SET NOT NULL;
        ALTER TABLE public.meta_webhook_buffer
            ADD CONSTRAINT meta_webhook_buffer_field_check CHECK (field <> '');
        ALTER TABLE public.meta_webhook_buffer
            ADD CONSTRAINT meta_webhook_buffer_status_check
            CHECK (status IN ('pending', 'processing', 'completed', 'failed'));
    END IF;
END
$$;

-- NOTE on key-namespace continuity: migrated rows keep their legacy event_key
-- format. A post-migration Meta replay of the same logical event therefore
-- creates one additional buffered row under the revised key format. This is
-- accepted by design: downstream desktop sync performs payload-level
-- provider_message_id / comment_id deduplication before rendering, so no
-- duplicate reaches user-visible state.
