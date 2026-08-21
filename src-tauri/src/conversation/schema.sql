CREATE TABLE IF NOT EXISTS channel_accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel TEXT NOT NULL CHECK (channel IN ('zalo', 'whatsapp', 'messenger', 'email')),
    provider_account_key TEXT NOT NULL,
    display_name TEXT,
    avatar_url TEXT,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    connection_status TEXT NOT NULL DEFAULT 'disconnected',
    last_connected_at TEXT,
    last_error TEXT,
    capabilities_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(channel, provider_account_key)
);

CREATE TABLE IF NOT EXISTS channel_identities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    lead_id INTEGER,
    channel TEXT NOT NULL CHECK (channel IN ('zalo', 'whatsapp', 'messenger', 'email')),
    channel_account_id INTEGER,
    identity_namespace TEXT NOT NULL,
    external_id TEXT NOT NULL,
    normalized_address TEXT,
    display_name TEXT,
    avatar_url TEXT,
    profile_synced_at TEXT,
    profile_sync_error TEXT,
    profile_sync_attempts INTEGER NOT NULL DEFAULT 0,
    next_profile_retry_at TEXT,
    link_source TEXT NOT NULL DEFAULT 'unlinked',
    link_confidence REAL,
    verified_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(lead_id) REFERENCES leads(id) ON DELETE SET NULL,
    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_channel_identities_lead ON channel_identities(lead_id, channel);
CREATE UNIQUE INDEX IF NOT EXISTS idx_channel_identities_legacy_unique 
ON channel_identities(channel, identity_namespace, external_id) 
WHERE channel_account_id IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_channel_identities_page_unique 
ON channel_identities(channel, channel_account_id, identity_namespace, external_id) 
WHERE channel_account_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS channel_conversations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel_account_id INTEGER NOT NULL,
    external_thread_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('direct', 'group', 'email_thread')),
    title TEXT,
    avatar_url TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    last_message_at TEXT,
    last_synced_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(channel_account_id, external_thread_id, kind),
    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS conversation_participants (
    conversation_id INTEGER NOT NULL,
    identity_id INTEGER NOT NULL,
    participant_role TEXT NOT NULL DEFAULT 'member',
    joined_at TEXT,
    left_at TEXT,
    PRIMARY KEY(conversation_id, identity_id),
    FOREIGN KEY(conversation_id) REFERENCES channel_conversations(id) ON DELETE CASCADE,
    FOREIGN KEY(identity_id) REFERENCES channel_identities(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL,
    sender_identity_id INTEGER,
    direction TEXT NOT NULL CHECK (direction IN ('inbound', 'outbound', 'system')),
    provider_message_id TEXT,
    provider_client_message_id TEXT,
    local_request_id TEXT,
    content_type TEXT NOT NULL,
    text_content TEXT,
    content_json TEXT NOT NULL DEFAULT '{}',
    raw_payload_json TEXT,
    provider_timestamp TEXT,
    ingested_at TEXT NOT NULL,
    sent_at TEXT,
    edited_at TEXT,
    removed_at TEXT,
    status TEXT NOT NULL DEFAULT 'received',
    error_code TEXT,
    error_message TEXT,
    reply_to_message_id INTEGER,
    reply_sender_identity_id INTEGER,
    FOREIGN KEY(conversation_id) REFERENCES channel_conversations(id) ON DELETE CASCADE,
    FOREIGN KEY(sender_identity_id) REFERENCES channel_identities(id) ON DELETE SET NULL,
    FOREIGN KEY(reply_to_message_id) REFERENCES messages(id) ON DELETE SET NULL,
    FOREIGN KEY(reply_sender_identity_id) REFERENCES channel_identities(id) ON DELETE SET NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_provider_unique
ON messages(conversation_id, provider_message_id)
WHERE provider_message_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_local_request_unique
ON messages(local_request_id)
WHERE local_request_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_messages_timeline
ON messages(conversation_id, provider_timestamp, ingested_at, id);

CREATE TABLE IF NOT EXISTS message_attachments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id INTEGER NOT NULL,
    provider_attachment_id TEXT,
    attachment_type TEXT NOT NULL,
    file_name TEXT,
    mime_type TEXT,
    size_bytes INTEGER,
    remote_url TEXT,
    local_path TEXT,
    thumbnail_url TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_message_attachments_remote
ON message_attachments(message_id, remote_url)
WHERE remote_url IS NOT NULL;

CREATE TABLE IF NOT EXISTS message_reactions (
    message_id INTEGER NOT NULL,
    actor_external_id TEXT NOT NULL,
    reaction TEXT NOT NULL,
    count INTEGER NOT NULL DEFAULT 1,
    reacted_at TEXT,
    removed_at TEXT,
    PRIMARY KEY(message_id, actor_external_id, reaction),
    FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS message_receipts (
    message_id INTEGER NOT NULL,
    recipient_external_id TEXT NOT NULL DEFAULT '',
    receipt_type TEXT NOT NULL CHECK (receipt_type IN ('accepted', 'sent', 'delivered', 'read', 'failed')),
    occurred_at TEXT NOT NULL,
    raw_payload_json TEXT,
    PRIMARY KEY(message_id, recipient_external_id, receipt_type),
    FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS channel_sync_checkpoints (
    channel_account_id INTEGER NOT NULL,
    scope TEXT NOT NULL,
    cursor TEXT,
    last_event_at TEXT,
    last_success_at TEXT,
    reconnect_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(channel_account_id, scope),
    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS channel_account_migration_markers (
    channel_account_id INTEGER NOT NULL,
    migration_key TEXT NOT NULL,
    applied_at TEXT NOT NULL,
    PRIMARY KEY(channel_account_id, migration_key),
    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS pending_message_reactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel_account_id INTEGER NOT NULL,
    external_thread_id TEXT NOT NULL,
    conversation_kind TEXT NOT NULL CHECK (conversation_kind IN ('direct', 'group')),
    provider_event_id TEXT NOT NULL DEFAULT '',
    provider_message_id TEXT NOT NULL DEFAULT '',
    provider_client_message_id TEXT NOT NULL DEFAULT '',
    actor_external_id TEXT NOT NULL,
    reaction TEXT NOT NULL,
    removed INTEGER NOT NULL CHECK (removed IN (0, 1)),
    occurred_at TEXT NOT NULL,
    raw_payload_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(
        channel_account_id,
        external_thread_id,
        conversation_kind,
        provider_event_id,
        provider_message_id,
        provider_client_message_id,
        actor_external_id,
        reaction,
        removed
    ),
    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_pending_message_reactions_provider_message
ON pending_message_reactions(
    channel_account_id,
    external_thread_id,
    conversation_kind,
    provider_message_id,
    occurred_at,
    id
);

CREATE INDEX IF NOT EXISTS idx_pending_message_reactions_provider_client
ON pending_message_reactions(
    channel_account_id,
    external_thread_id,
    conversation_kind,
    provider_client_message_id,
    occurred_at,
    id
);

CREATE TABLE IF NOT EXISTS message_outbox_jobs (
    id TEXT PRIMARY KEY,
    message_id INTEGER NOT NULL UNIQUE,
    channel_account_id INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'retry_wait', 'completed', 'failed', 'cancelled')),
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE,
    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_channel_identities_lead
ON channel_identities(lead_id, channel);

CREATE INDEX IF NOT EXISTS idx_conversations_last_message
ON channel_conversations(last_message_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_outbox_claim
ON message_outbox_jobs(status, next_attempt_at, created_at);

CREATE TABLE IF NOT EXISTS message_send_batches (
    id TEXT PRIMARY KEY,
    conversation_id INTEGER NOT NULL,
    channel_account_id INTEGER NOT NULL,
    client_batch_id TEXT NOT NULL,
    reply_to_message_id INTEGER,
    part_count INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN ('queued', 'sending', 'partial', 'sent', 'failed')),
    error_code TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    UNIQUE(channel_account_id, client_batch_id),
    FOREIGN KEY(conversation_id) REFERENCES channel_conversations(id) ON DELETE CASCADE,
    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE,
    FOREIGN KEY(reply_to_message_id) REFERENCES messages(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_send_batches_conversation_created
ON message_send_batches(conversation_id, created_at, id);

CREATE TABLE IF NOT EXISTS conversation_staged_attachments (
    id TEXT PRIMARY KEY,
    conversation_id INTEGER NOT NULL,
    staged_attachment_id TEXT NOT NULL UNIQUE,
    media_type TEXT NOT NULL CHECK (media_type IN ('image', 'video', 'audio', 'file')),
    mime_type TEXT NOT NULL,
    file_name TEXT NOT NULL,
    file_size_bytes INTEGER NOT NULL,
    local_path TEXT NOT NULL,
    thumbnail_path TEXT,
    width INTEGER,
    height INTEGER,
    duration_seconds REAL,
    status TEXT NOT NULL DEFAULT 'staged' CHECK (status IN ('staged', 'claimed', 'discarded', 'expired')),
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    claimed_at TEXT,
    FOREIGN KEY(conversation_id) REFERENCES channel_conversations(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_staged_attachments_cleanup
ON conversation_staged_attachments(status, expires_at, created_at);

CREATE TABLE IF NOT EXISTS conversation_quick_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    shortcut TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    body_text TEXT NOT NULL,
    category TEXT,
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    usage_count INTEGER NOT NULL DEFAULT 0,
    last_used_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_quick_templates_active_shortcut
ON conversation_quick_templates(is_active, shortcut);

CREATE TABLE IF NOT EXISTS message_mentions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id INTEGER NOT NULL,
    identity_id INTEGER,
    external_id TEXT,
    display_text TEXT NOT NULL,
    start_utf16 INTEGER NOT NULL,
    length_utf16 INTEGER NOT NULL CHECK (length_utf16 > 0),
    created_at TEXT NOT NULL,
    UNIQUE(message_id, start_utf16, length_utf16),
    FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE,
    FOREIGN KEY(identity_id) REFERENCES channel_identities(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_message_mentions_message
ON message_mentions(message_id);

CREATE TABLE IF NOT EXISTS reaction_mutations (
    client_mutation_id TEXT PRIMARY KEY,
    message_id INTEGER NOT NULL,
    reaction_code TEXT NOT NULL,
    active INTEGER NOT NULL CHECK (active IN (0, 1)),
    count INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'running' CHECK (status IN ('running', 'succeeded', 'failed', 'delivery_unknown')),
    error_code TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_reaction_mutations_message
ON reaction_mutations(message_id, created_at);

CREATE TABLE IF NOT EXISTS message_forward_operations (
    client_operation_id TEXT PRIMARY KEY,
    source_message_id INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN ('queued', 'sending', 'partial', 'sent', 'failed', 'delivery_unknown')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(source_message_id) REFERENCES messages(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_forward_operations_source
ON message_forward_operations(source_message_id);

CREATE TABLE IF NOT EXISTS message_forward_targets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_operation_id TEXT NOT NULL,
    target_conversation_id INTEGER NOT NULL,
    send_batch_id TEXT,
    status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN ('queued', 'sending', 'sent', 'failed', 'delivery_unknown')),
    error_code TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(client_operation_id, target_conversation_id),
    FOREIGN KEY(client_operation_id) REFERENCES message_forward_operations(client_operation_id) ON DELETE CASCADE,
    FOREIGN KEY(target_conversation_id) REFERENCES channel_conversations(id) ON DELETE CASCADE,
    FOREIGN KEY(send_batch_id) REFERENCES message_send_batches(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_forward_targets_operation
ON message_forward_targets(client_operation_id, status);

CREATE TABLE IF NOT EXISTS conversation_pinned_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL,
    message_id INTEGER NOT NULL,
    pinned_by TEXT,
    pinned_at TEXT NOT NULL,
    UNIQUE(conversation_id, message_id),
    FOREIGN KEY(conversation_id) REFERENCES channel_conversations(id) ON DELETE CASCADE,
    FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_pinned_messages_conversation
ON conversation_pinned_messages(conversation_id, pinned_at DESC);

CREATE TABLE IF NOT EXISTS deleted_conversations (
    channel_account_id INTEGER NOT NULL,
    external_thread_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    deleted_at TEXT NOT NULL,
    PRIMARY KEY(channel_account_id, external_thread_id, kind)
);

CREATE TABLE IF NOT EXISTS pending_spool_cleanups (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    local_path TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS facebook_page_tokens (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel_account_id INTEGER NOT NULL UNIQUE,
    page_id TEXT NOT NULL UNIQUE,
    access_token_encrypted TEXT NOT NULL,
    token_status TEXT NOT NULL DEFAULT 'valid' CHECK (token_status IN ('valid', 'invalid', 'revoked')),
    granted_scopes_json TEXT NOT NULL DEFAULT '[]',
    last_validated_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS channel_sync_dead_letters (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel_account_id INTEGER NOT NULL,
    scope TEXT NOT NULL,
    event_id INTEGER NOT NULL,
    item_index INTEGER NOT NULL DEFAULT 0,
    event_type TEXT NOT NULL,
    event_key TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    error_reason TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'retrying', 'resolved', 'ignored', 'failed')),
    attempts INTEGER NOT NULL DEFAULT 0,
    next_retry_at TEXT,
    owner_id TEXT,
    claim_deadline TEXT,
    resolved_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE,
    UNIQUE(channel_account_id, scope, event_id, item_index)
);

CREATE INDEX IF NOT EXISTS idx_dead_letters_account_scope ON channel_sync_dead_letters(channel_account_id, scope);
CREATE INDEX IF NOT EXISTS idx_dead_letters_status ON channel_sync_dead_letters(status, next_retry_at);
