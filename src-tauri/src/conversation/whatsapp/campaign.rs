use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RecipientEvaluation {
    Eligible {
        phone_e164: String,
        jid: String,
        rendered_text: String,
    },
    Skipped {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCampaignRequest {
    pub name: String,
    pub template_id: Option<i64>,
    pub assigned_account_ids: Vec<i64>,
    pub rate_limit_min_delay_secs: Option<i64>,
    pub rate_limit_max_delay_secs: Option<i64>,
    pub burst_limit_per_batch: Option<i64>,
    pub burst_pause_secs: Option<i64>,
    pub quiet_hours_enabled: Option<bool>,
    pub quiet_hours_start: Option<String>,
    pub quiet_hours_end: Option<String>,
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignRecipientInput {
    pub lead_id: Option<i64>,
    pub recipient_phone_raw: String,
    pub template: Option<String>,
    pub lead_vars: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignRecipientRecord {
    pub id: i64,
    pub campaign_id: i64,
    pub lead_id: Option<i64>,
    pub recipient_phone_raw: String,
    pub recipient_phone_e164: String,
    pub recipient_jid: String,
    pub assigned_channel_account_id: Option<i64>,
    pub status: String,
    pub skip_reason: Option<String>,
    pub rendered_text: Option<String>,
    pub rendered_image_url: Option<String>,
    pub message_id: Option<i64>,
    pub outbox_job_id: Option<String>,
    pub attempt_count: i64,
    pub last_attempt_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignCounters {
    pub total_recipients: i64,
    pub sent_count: i64,
    pub delivered_count: i64,
    pub read_count: i64,
    pub failed_count: i64,
    pub skipped_count: i64,
}

/// Evaluates whether a given HH:MM timestamp falls within quiet hours,
/// properly handling wrap-around midnight (e.g. 22:00 -> 07:30).
pub fn is_in_quiet_hours(start_hhmm: &str, end_hhmm: &str, current_hhmm: &str) -> bool {
    let parse_minutes = |s: &str| -> Option<u32> {
        let parts: Vec<&str> = s.trim().split(':').collect();
        if parts.len() < 2 {
            return None;
        }
        let h: u32 = parts[0].parse().ok()?;
        let m: u32 = parts[1].parse().ok()?;
        if h >= 24 || m >= 60 {
            return None;
        }
        Some(h * 60 + m)
    };

    let start = match parse_minutes(start_hhmm) {
        Some(m) => m,
        None => return false,
    };
    let end = match parse_minutes(end_hhmm) {
        Some(m) => m,
        None => return false,
    };
    let current = match parse_minutes(current_hhmm) {
        Some(m) => m,
        None => return false,
    };

    if start == end {
        return false;
    }

    if start < end {
        current >= start && current < end
    } else {
        // Wrap around midnight (e.g. 22:00 -> 07:30)
        current >= start || current < end
    }
}

/// Adds a phone number or email to the contact suppression list.
pub fn add_to_suppression_list(
    conn: &rusqlite::Connection,
    channel: &str,
    phone_or_email: &str,
    reason: &str,
    source: &str,
) -> rusqlite::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO contact_suppression_list (channel, phone_or_email, reason, source, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(channel, phone_or_email) DO UPDATE SET
             reason = excluded.reason,
             source = excluded.source,
             created_at = excluded.created_at",
        rusqlite::params![channel, phone_or_email.trim(), reason, source, now],
    )?;
    Ok(())
}

/// Checks if a phone or email is in the suppression list for a specific channel.
pub fn is_suppressed(
    conn: &rusqlite::Connection,
    channel: &str,
    phone_or_email: &str,
) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare_cached(
        "SELECT reason FROM contact_suppression_list 
         WHERE channel = ?1 AND phone_or_email = ?2",
    )?;
    let mut rows = stmt.query(rusqlite::params![channel, phone_or_email.trim()])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row.get(0)?))
    } else {
        Ok(None)
    }
}

#[derive(Debug, Default, Clone)]
pub struct WhatsAppCampaignEngine;

impl WhatsAppCampaignEngine {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates recipient eligibility, performing E.164 normalization, WhatsApp direct JID formatting,
    /// suppression list verification, and spintax template rendering.
    pub fn evaluate_recipient(
        &self,
        conn: &rusqlite::Connection,
        phone_raw: &str,
        template: &str,
        lead_vars: &HashMap<String, String>,
    ) -> RecipientEvaluation {
        // 1. Normalize phone to E.164 (default Vietnam country code 84)
        let phone_e164 = match super::phone::normalize_e164(phone_raw, Some("84")) {
            Some(digits) => digits,
            None => {
                return RecipientEvaluation::Skipped {
                    reason: "invalid_phone_number".to_string(),
                }
            }
        };

        // 2. Direct WhatsApp JID
        let jid = super::phone::phone_to_direct_jid(&phone_e164);

        // 3. Suppression check
        let suppressed_reason: Option<String> = conn
            .query_row(
                "SELECT reason FROM contact_suppression_list 
                 WHERE channel = 'whatsapp' 
                   AND (phone_or_email = ?1 OR phone_or_email = ?2 OR phone_or_email = ?3)
                 LIMIT 1",
                rusqlite::params![phone_e164, format!("+{}", phone_e164), phone_raw.trim(),],
                |r| r.get(0),
            )
            .ok();

        if let Some(reason) = suppressed_reason {
            let skip_reason = if reason.ends_with("_suppressed") || reason.contains("suppressed") {
                reason
            } else {
                format!("{reason}_suppressed")
            };
            return RecipientEvaluation::Skipped {
                reason: skip_reason,
            };
        }

        // 4. Render template with Spintax
        let rendered_text = super::spintax::render_template(template, lead_vars);

        RecipientEvaluation::Eligible {
            phone_e164,
            jid,
            rendered_text,
        }
    }
}

/// Creates a new WhatsApp outreach campaign in 'draft' status.
pub fn create_campaign(
    conn: &rusqlite::Connection,
    req: &CreateCampaignRequest,
) -> rusqlite::Result<i64> {
    let now = chrono::Utc::now().to_rfc3339();
    let accounts_json =
        serde_json::to_string(&req.assigned_account_ids).unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT INTO whatsapp_outreach_campaigns (
            name, template_id, status, assigned_account_ids_json,
            rate_limit_min_delay_secs, rate_limit_max_delay_secs,
            burst_limit_per_batch, burst_pause_secs,
            quiet_hours_enabled, quiet_hours_start, quiet_hours_end,
            timezone, created_at, updated_at
         ) VALUES (?1, ?2, 'draft', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
        rusqlite::params![
            req.name.trim(),
            req.template_id,
            accounts_json,
            req.rate_limit_min_delay_secs.unwrap_or(8),
            req.rate_limit_max_delay_secs.unwrap_or(20),
            req.burst_limit_per_batch.unwrap_or(25),
            req.burst_pause_secs.unwrap_or(60),
            if req.quiet_hours_enabled.unwrap_or(true) {
                1
            } else {
                0
            },
            req.quiet_hours_start.as_deref().unwrap_or("22:00"),
            req.quiet_hours_end.as_deref().unwrap_or("07:30"),
            req.timezone.as_deref().unwrap_or("Asia/Ho_Chi_Minh"),
            now
        ],
    )?;

    Ok(conn.last_insert_rowid())
}

/// Populates recipients into a campaign, performing eligibility evaluation and deduplication.
pub fn populate_recipients(
    conn: &rusqlite::Connection,
    campaign_id: i64,
    recipients: &[CampaignRecipientInput],
    template_override: Option<&str>,
) -> rusqlite::Result<usize> {
    let engine = WhatsAppCampaignEngine::new();
    let now = chrono::Utc::now().to_rfc3339();
    let mut inserted_count = 0;

    for r in recipients {
        let template_str = r.template.as_deref().or(template_override).unwrap_or("");
        let empty_vars = HashMap::new();
        let vars = r.lead_vars.as_ref().unwrap_or(&empty_vars);

        let eval = engine.evaluate_recipient(conn, &r.recipient_phone_raw, template_str, vars);

        let (status, skip_reason, phone_e164, jid, rendered_text) = match eval {
            RecipientEvaluation::Eligible {
                phone_e164,
                jid,
                rendered_text,
            } => ("pending", None, phone_e164, jid, Some(rendered_text)),
            RecipientEvaluation::Skipped { reason } => {
                let e164 = super::phone::normalize_e164(&r.recipient_phone_raw, Some("84"))
                    .unwrap_or_else(|| format!("invalid_{}", &r.recipient_phone_raw));
                let jid = super::phone::phone_to_direct_jid(&e164);
                ("skipped", Some(reason), e164, jid, None)
            }
        };

        let rows = conn.execute(
            "INSERT OR IGNORE INTO whatsapp_campaign_recipients (
                campaign_id, lead_id, recipient_phone_raw, recipient_phone_e164, recipient_jid,
                status, skip_reason, rendered_text, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            rusqlite::params![
                campaign_id,
                r.lead_id,
                r.recipient_phone_raw,
                phone_e164,
                jid,
                status,
                skip_reason,
                rendered_text,
                now
            ],
        )?;

        if rows > 0 {
            inserted_count += 1;
        }
    }

    refresh_campaign_counters(conn, campaign_id)?;
    Ok(inserted_count)
}

/// Atomically claims the next pending batch of recipients for a campaign and transitions them to 'running'.
pub fn claim_next_recipients(
    conn: &rusqlite::Connection,
    campaign_id: i64,
    limit: usize,
    assigned_account_id: Option<i64>,
) -> rusqlite::Result<Vec<CampaignRecipientRecord>> {
    let now = chrono::Utc::now().to_rfc3339();

    let mut stmt = conn.prepare(
        "SELECT id, campaign_id, lead_id, recipient_phone_raw, recipient_phone_e164, recipient_jid,
                assigned_channel_account_id, status, skip_reason, rendered_text, rendered_image_url,
                message_id, outbox_job_id, attempt_count, last_attempt_at, last_error, created_at, updated_at
         FROM whatsapp_campaign_recipients
         WHERE campaign_id = ?1 AND status = 'pending'
         ORDER BY id ASC
         LIMIT ?2",
    )?;

    let mut records: Vec<CampaignRecipientRecord> = stmt
        .query_map(rusqlite::params![campaign_id, limit as i64], |r| {
            Ok(CampaignRecipientRecord {
                id: r.get(0)?,
                campaign_id: r.get(1)?,
                lead_id: r.get(2)?,
                recipient_phone_raw: r.get(3)?,
                recipient_phone_e164: r.get(4)?,
                recipient_jid: r.get(5)?,
                assigned_channel_account_id: r.get(6)?,
                status: r.get(7)?,
                skip_reason: r.get(8)?,
                rendered_text: r.get(9)?,
                rendered_image_url: r.get(10)?,
                message_id: r.get(11)?,
                outbox_job_id: r.get(12)?,
                attempt_count: r.get(13)?,
                last_attempt_at: r.get(14)?,
                last_error: r.get(15)?,
                created_at: r.get(16)?,
                updated_at: r.get(17)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    for item in &mut records {
        conn.execute(
            "UPDATE whatsapp_campaign_recipients
             SET status = 'running',
                 assigned_channel_account_id = ?1,
                 attempt_count = attempt_count + 1,
                 last_attempt_at = ?2,
                 updated_at = ?2
             WHERE id = ?3",
            rusqlite::params![assigned_account_id, now, item.id],
        )?;
        item.status = "running".to_string();
        item.assigned_channel_account_id = assigned_account_id;
        item.attempt_count += 1;
        item.last_attempt_at = Some(now.clone());
        item.updated_at = now.clone();
    }

    Ok(records)
}

/// Updates the status and details of a campaign recipient, and synchronizes campaign counters.
pub fn update_recipient_status(
    conn: &rusqlite::Connection,
    recipient_id: i64,
    status: &str,
    skip_reason: Option<&str>,
    last_error: Option<&str>,
    message_id: Option<i64>,
    outbox_job_id: Option<&str>,
) -> rusqlite::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let campaign_id: i64 = conn.query_row(
        "UPDATE whatsapp_campaign_recipients
         SET status = ?1,
             skip_reason = COALESCE(?2, skip_reason),
             last_error = COALESCE(?3, last_error),
             message_id = COALESCE(?4, message_id),
             outbox_job_id = COALESCE(?5, outbox_job_id),
             updated_at = ?6
         WHERE id = ?7
         RETURNING campaign_id",
        rusqlite::params![
            status,
            skip_reason,
            last_error,
            message_id,
            outbox_job_id,
            now,
            recipient_id
        ],
        |r| r.get(0),
    )?;

    refresh_campaign_counters(conn, campaign_id)?;
    Ok(())
}

/// Recalculates and updates aggregated counters in `whatsapp_outreach_campaigns`.
pub fn refresh_campaign_counters(
    conn: &rusqlite::Connection,
    campaign_id: i64,
) -> rusqlite::Result<CampaignCounters> {
    let now = chrono::Utc::now().to_rfc3339();

    let mut stmt = conn.prepare(
        "SELECT 
            COUNT(*),
            SUM(CASE WHEN status IN ('sent', 'delivered', 'read') THEN 1 ELSE 0 END),
            SUM(CASE WHEN status IN ('delivered', 'read') THEN 1 ELSE 0 END),
            SUM(CASE WHEN status = 'read' THEN 1 ELSE 0 END),
            SUM(CASE WHEN status IN ('failed', 'blocked', 'delivery_unknown') THEN 1 ELSE 0 END),
            SUM(CASE WHEN status = 'skipped' THEN 1 ELSE 0 END),
            SUM(CASE WHEN status IN ('pending', 'running') THEN 1 ELSE 0 END)
         FROM whatsapp_campaign_recipients
         WHERE campaign_id = ?1",
    )?;

    let (total, sent, delivered, read, failed, skipped, active_pending_or_running): (
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
    ) = stmt.query_row(rusqlite::params![campaign_id], |r| {
        Ok((
            r.get::<_, Option<i64>>(0)?.unwrap_or(0),
            r.get::<_, Option<i64>>(1)?.unwrap_or(0),
            r.get::<_, Option<i64>>(2)?.unwrap_or(0),
            r.get::<_, Option<i64>>(3)?.unwrap_or(0),
            r.get::<_, Option<i64>>(4)?.unwrap_or(0),
            r.get::<_, Option<i64>>(5)?.unwrap_or(0),
            r.get::<_, Option<i64>>(6)?.unwrap_or(0),
        ))
    })?;

    conn.execute(
        "UPDATE whatsapp_outreach_campaigns
         SET total_recipients = ?1,
             sent_count = ?2,
             delivered_count = ?3,
             read_count = ?4,
             failed_count = ?5,
             skipped_count = ?6,
             status = CASE 
                 WHEN ?7 = 0 AND total_recipients > 0 AND status IN ('running', 'queued') THEN 'completed'
                 ELSE status
             END,
             completed_at = CASE
                 WHEN ?7 = 0 AND total_recipients > 0 AND status IN ('running', 'queued') THEN COALESCE(completed_at, ?8)
                 ELSE completed_at
             END,
             updated_at = ?8
         WHERE id = ?9",
        rusqlite::params![
            total,
            sent,
            delivered,
            read,
            failed,
            skipped,
            active_pending_or_running,
            now,
            campaign_id
        ],
    )?;

    Ok(CampaignCounters {
        total_recipients: total,
        sent_count: sent,
        delivered_count: delivered,
        read_count: read,
        failed_count: failed,
        skipped_count: skipped,
    })
}

/// Pauses an active or queued campaign.
pub fn pause_campaign(conn: &rusqlite::Connection, campaign_id: i64) -> rusqlite::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE whatsapp_outreach_campaigns 
         SET status = 'paused', updated_at = ?1 
         WHERE id = ?2 AND status IN ('running', 'queued', 'draft')",
        rusqlite::params![now, campaign_id],
    )?;
    Ok(())
}

/// Resumes a paused campaign.
pub fn resume_campaign(conn: &rusqlite::Connection, campaign_id: i64) -> rusqlite::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE whatsapp_outreach_campaigns 
         SET status = 'queued', updated_at = ?1 
         WHERE id = ?2 AND status IN ('paused', 'draft')",
        rusqlite::params![now, campaign_id],
    )?;
    Ok(())
}

/// Cancels a campaign and marks all remaining pending recipients as skipped.
pub fn cancel_campaign(conn: &rusqlite::Connection, campaign_id: i64) -> rusqlite::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE whatsapp_outreach_campaigns 
         SET status = 'cancelled', updated_at = ?1 
         WHERE id = ?2",
        rusqlite::params![now, campaign_id],
    )?;
    conn.execute(
        "UPDATE whatsapp_campaign_recipients
         SET status = 'skipped', skip_reason = 'campaign_cancelled', updated_at = ?1
         WHERE campaign_id = ?2 AND status = 'pending'",
        rusqlite::params![now, campaign_id],
    )?;
    refresh_campaign_counters(conn, campaign_id)?;
    Ok(())
}

/// Recovers an interrupted campaign after crash or application restart (REQ-WA-10).
/// Resets 'running' recipients without dispatched message records back to 'pending'.
pub fn recover_interrupted_campaign(
    conn: &rusqlite::Connection,
    campaign_id: i64,
) -> rusqlite::Result<usize> {
    let now = chrono::Utc::now().to_rfc3339();
    let recovered = conn.execute(
        "UPDATE whatsapp_campaign_recipients
         SET status = 'pending',
             assigned_channel_account_id = NULL,
             updated_at = ?1
         WHERE campaign_id = ?2 
           AND status = 'running' 
           AND message_id IS NULL 
           AND outbox_job_id IS NULL",
        rusqlite::params![now, campaign_id],
    )?;
    refresh_campaign_counters(conn, campaign_id)?;
    Ok(recovered)
}
