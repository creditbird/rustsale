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
--      (drop old status CHECK -> map values -> RENAME event_type -> field ->
--      re-key rows into a collision-proof 'legacy_<family>_<key>' namespace ->
--      normalize field values -> swap indexes/constraints).
--      Every legacy row survives: collisions are resolved by deterministic
--      re-keying, never by deletion.
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

        -- 3. Rename FIRST: every subsequent statement operates on 'field'.
        ALTER TABLE public.meta_webhook_buffer RENAME COLUMN event_type TO field;

        -- 4. Re-key EVERY legacy row into a collision-proof namespace that
        --    preserves the ORIGINAL event family. Legacy uniqueness was
        --    (page_id, event_type, event_key); embedding the pre-normalized
        --    field value keeps that distinction, so NO row is ever deleted.
        UPDATE public.meta_webhook_buffer
           SET event_key = 'legacy_' || field || '_' || event_key;

        -- 5. NOW normalize the field namespace. Legacy messaging-family
        --    events (echoes/deliveries/reads/reactions/messages) live under
        --    the canonical webhook field "messages"; feed/leadgen unchanged.
        UPDATE public.meta_webhook_buffer
           SET field = 'messages'
         WHERE field IN ('message_echoes', 'message_deliveries', 'message_reads', 'message_reactions', 'messages');

        -- 6. Replace legacy index structures with the revised ones. The
        --    legacy unique domain was strictly finer than the revised one
        --    after step 4, so the new unique index cannot fail.
        DROP INDEX IF EXISTS public.idx_meta_buffer_dedup;
        DROP INDEX IF EXISTS public.idx_meta_buffer_sync;
        CREATE UNIQUE INDEX IF NOT EXISTS idx_meta_buffer_dedup
            ON public.meta_webhook_buffer(tenant_id, page_id, event_key);
        CREATE INDEX IF NOT EXISTS idx_meta_buffer_queue
            ON public.meta_webhook_buffer(tenant_id, status, id);

        -- 7. Enforce revised column constraints on the transformed table.
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

-- NOTE on key namespaces: migrated rows carry 'legacy_<family>_<key>' keys.
-- A post-migration Meta replay of the same logical event arrives under the
-- revised canonical key format and buffers as one additional row. This is
-- accepted by design: downstream desktop sync performs payload-level
-- provider_message_id / comment_id deduplication before rendering, so no
-- duplicate reaches user-visible state.
