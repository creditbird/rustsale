use rusqlite::{Connection, Result};

pub fn apply_whatsapp_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS whatsapp_sessions (
            channel_account_id INTEGER PRIMARY KEY REFERENCES channel_accounts(id) ON DELETE RESTRICT,
            account_key TEXT NOT NULL UNIQUE,
            phone_number TEXT NOT NULL,
            lid TEXT,
            push_name TEXT,
            avatar_url TEXT,
            session_db_filename TEXT NOT NULL,
            is_valid INTEGER NOT NULL DEFAULT 1 CHECK (is_valid IN (0, 1)),
            last_connected_at TEXT,
            last_error TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_whatsapp_sessions_phone
        ON whatsapp_sessions(phone_number);

        CREATE TABLE IF NOT EXISTS whatsapp_lid_pn_mappings (
            lid TEXT PRIMARY KEY,
            phone_e164 TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_whatsapp_lid_pn_phone
        ON whatsapp_lid_pn_mappings(phone_e164);

        CREATE TABLE IF NOT EXISTS contact_suppression_list (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            channel TEXT NOT NULL CHECK (channel IN ('whatsapp', 'zalo', 'email', 'messenger')),
            phone_or_email TEXT NOT NULL,
            reason TEXT NOT NULL,
            source TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(channel, phone_or_email)
        );

        CREATE INDEX IF NOT EXISTS idx_suppression_lookup
        ON contact_suppression_list(channel, phone_or_email);

        CREATE TABLE IF NOT EXISTS whatsapp_outreach_campaigns (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            template_id INTEGER,
            status TEXT NOT NULL CHECK (status IN ('draft', 'queued', 'running', 'pausing', 'paused', 'completed', 'partial', 'failed', 'cancelled')),
            total_recipients INTEGER NOT NULL DEFAULT 0,
            sent_count INTEGER NOT NULL DEFAULT 0,
            delivered_count INTEGER NOT NULL DEFAULT 0,
            read_count INTEGER NOT NULL DEFAULT 0,
            failed_count INTEGER NOT NULL DEFAULT 0,
            skipped_count INTEGER NOT NULL DEFAULT 0,
            assigned_account_ids_json TEXT NOT NULL DEFAULT '[]',
            rate_limit_min_delay_secs INTEGER NOT NULL DEFAULT 8,
            rate_limit_max_delay_secs INTEGER NOT NULL DEFAULT 20,
            burst_limit_per_batch INTEGER NOT NULL DEFAULT 25,
            burst_pause_secs INTEGER NOT NULL DEFAULT 60,
            quiet_hours_enabled INTEGER NOT NULL DEFAULT 1 CHECK (quiet_hours_enabled IN (0, 1)),
            quiet_hours_start TEXT NOT NULL DEFAULT '22:00',
            quiet_hours_end TEXT NOT NULL DEFAULT '07:30',
            timezone TEXT NOT NULL DEFAULT 'Asia/Ho_Chi_Minh',
            last_heartbeat_at TEXT,
            started_at TEXT,
            completed_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_wa_outreach_campaigns_status
        ON whatsapp_outreach_campaigns(status, updated_at);

        CREATE TABLE IF NOT EXISTS whatsapp_campaign_recipients (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            campaign_id INTEGER NOT NULL REFERENCES whatsapp_outreach_campaigns(id) ON DELETE CASCADE,
            lead_id INTEGER REFERENCES leads(id) ON DELETE SET NULL,
            recipient_phone_raw TEXT NOT NULL,
            recipient_phone_e164 TEXT NOT NULL,
            recipient_jid TEXT NOT NULL,
            assigned_channel_account_id INTEGER REFERENCES channel_accounts(id) ON DELETE SET NULL,
            status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'sent', 'delivered', 'read', 'failed', 'skipped', 'blocked', 'delivery_unknown')),
            skip_reason TEXT,
            rendered_text TEXT,
            rendered_image_url TEXT,
            message_id INTEGER REFERENCES messages(id) ON DELETE SET NULL,
            outbox_job_id TEXT REFERENCES message_outbox_jobs(id) ON DELETE SET NULL,
            attempt_count INTEGER NOT NULL DEFAULT 0,
            last_attempt_at TEXT,
            last_error TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(campaign_id, recipient_phone_e164)
        );

        CREATE INDEX IF NOT EXISTS idx_wa_campaign_recipients_claim
        ON whatsapp_campaign_recipients(campaign_id, status, id);
        "#,
    )?;

    Ok(())
}
