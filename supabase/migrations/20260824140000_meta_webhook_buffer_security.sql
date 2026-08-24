-- Strict tenant isolation and revoke insecure anon access on meta_webhook_buffer
DROP POLICY IF EXISTS "Allow anon select for realtime" ON meta_webhook_buffer;

REVOKE ALL ON TABLE meta_webhook_buffer FROM PUBLIC, anon;
GRANT SELECT, UPDATE(status) ON TABLE meta_webhook_buffer TO authenticated;
GRANT ALL ON TABLE meta_webhook_buffer TO service_role;

-- Ensure RLS is active
ALTER TABLE meta_webhook_buffer ENABLE ROW LEVEL SECURITY;

-- Ensure authenticated tenant policy
DROP POLICY IF EXISTS "Tenants select own buffered events" ON meta_webhook_buffer;
CREATE POLICY "Tenants select own buffered events" ON meta_webhook_buffer
    FOR SELECT TO authenticated
    USING (auth.uid() = tenant_id);

DROP POLICY IF EXISTS "Tenants ack own buffered events" ON meta_webhook_buffer;
CREATE POLICY "Tenants ack own buffered events" ON meta_webhook_buffer
    FOR UPDATE TO authenticated
    USING (auth.uid() = tenant_id AND status IN ('pending', 'processing'))
    WITH CHECK (auth.uid() = tenant_id AND status IN ('processing', 'completed', 'failed'));
