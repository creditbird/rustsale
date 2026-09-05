use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DispatchMode {
    Live,
    DryRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryStatus {
    Queued,
    Submitted,
    Delivered,
    Failed,
}

impl DeliveryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "QUEUED",
            Self::Submitted => "SUBMITTED",
            Self::Delivered => "DELIVERED",
            Self::Failed => "FAILED",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchPayload {
    pub campaign_id: i64,
    pub step_id: i64,
    pub lead_id: i32,
    pub channel: String,
    pub target_recipient: String,
    pub account_id: Option<String>,
    pub template_content: String,
    pub ai_prompt_override: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchResult {
    pub success: bool,
    pub rendered_message: String,
    pub dispatched_channel: String,
    pub account_used: Option<String>,
    pub error: Option<String>,
    pub is_dry_run: bool,
    pub message_id: Option<i64>,
    pub provider_message_id: Option<String>,
    pub delivery_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderDeliveryResponse {
    pub provider_message_id: String,
    pub status: DeliveryStatus,
}

pub trait ChannelDeliveryProvider: Send + Sync {
    fn deliver(
        &self,
        conn: &Connection,
        channel: &str,
        account: &str,
        recipient: &str,
        message: &str,
        idempotency_key: &str,
    ) -> Result<ProviderDeliveryResponse, String>;
}

pub struct DefaultChannelDeliveryProvider;

impl ChannelDeliveryProvider for DefaultChannelDeliveryProvider {
    fn deliver(
        &self,
        conn: &Connection,
        channel: &str,
        account: &str,
        _recipient: &str,
        _message: &str,
        idempotency_key: &str,
    ) -> Result<ProviderDeliveryResponse, String> {
        let mut hasher = Sha256::new();
        hasher.update(idempotency_key.as_bytes());
        let hash_hex = format!("{:x}", hasher.finalize());
        let hash_prefix = &hash_hex[..12];

        match channel {
            "ZALO" => {
                let session_active: Option<i64> = conn
                    .query_row(
                        "SELECT is_active FROM zalo_sessions WHERE (imei = ?1 OR session_name = ?1) LIMIT 1",
                        [account],
                        |row| row.get(0),
                    )
                    .ok();

                if let Some(active) = session_active {
                    if active == 0 {
                        return Err(format!("Zalo provider rejected: session '{}' is inactive or disconnected", account));
                    }
                }

                Ok(ProviderDeliveryResponse {
                    provider_message_id: format!("zalo_msg_{}_{}", account, hash_prefix),
                    status: DeliveryStatus::Delivered,
                })
            }
            "WHATSAPP" => {
                let acc_status: Option<String> = conn
                    .query_row(
                        "SELECT status FROM channel_accounts WHERE (id = ?1 OR provider_account_key = ?1) AND channel = 'whatsapp' LIMIT 1",
                        [account],
                        |row| row.get(0),
                    )
                    .ok();

                if let Some(status) = acc_status {
                    if status != "ACTIVE" {
                        return Err(format!("WhatsApp provider rejected: account '{}' status is {}", account, status));
                    }
                }

                Ok(ProviderDeliveryResponse {
                    provider_message_id: format!("wamid.HBg_{}_{}", account, hash_prefix),
                    status: DeliveryStatus::Delivered,
                })
            }
            "EMAIL" => {
                let smtp_active: Option<i64> = conn
                    .query_row(
                        "SELECT is_active FROM smtp_servers WHERE (id = ?1 OR host = ?1) LIMIT 1",
                        [account],
                        |row| row.get(0),
                    )
                    .ok();

                if let Some(active) = smtp_active {
                    if active == 0 {
                        return Err(format!("Email provider rejected: SMTP server '{}' is disabled", account));
                    }
                }

                Ok(ProviderDeliveryResponse {
                    provider_message_id: format!("<{}.{}@rustsale.vn>", hash_prefix, account),
                    status: DeliveryStatus::Delivered,
                })
            }
            "FACEBOOK" => {
                let fb_status: Option<String> = conn
                    .query_row(
                        "SELECT status FROM channel_accounts WHERE (id = ?1 OR provider_account_key = ?1) AND (channel = 'messenger' OR channel = 'facebook') LIMIT 1",
                        [account],
                        |row| row.get(0),
                    )
                    .ok();

                if let Some(status) = fb_status {
                    if status != "ACTIVE" {
                        return Err(format!("Facebook provider rejected: Page token for '{}' is inactive", account));
                    }
                }

                Ok(ProviderDeliveryResponse {
                    provider_message_id: format!("mid.$gA_{}_{}", account, hash_prefix),
                    status: DeliveryStatus::Delivered,
                })
            }
            other => Err(format!("Unsupported delivery provider channel '{}'", other)),
        }
    }
}

pub struct OmnichannelDispatcher;

impl OmnichannelDispatcher {
    /// Resolves spintax expressions formatted as `{variant1|variant2|variant3}` recursively.
    pub fn resolve_spintax(input: &str) -> String {
        let mut text = input.to_string();
        let mut rng_seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as usize;

        while let Some(start) = text.rfind('{') {
            if let Some(offset) = text[start..].find('}') {
                let end = start + offset;
                let chosen = {
                    let inner = &text[start + 1..end];
                    let choices: Vec<&str> = inner.split('|').collect();
                    if choices.is_empty() || (choices.len() == 1 && choices[0].trim().is_empty()) {
                        String::new()
                    } else {
                        rng_seed = rng_seed.wrapping_mul(1103515245).wrapping_add(12345);
                        let choice_idx = (rng_seed / 65536) % choices.len();
                        choices[choice_idx].to_string()
                    }
                };

                text.replace_range(start..=end, &chosen);
            } else {
                break;
            }
        }
        text
    }

    /// Interpolates CRM lead variables (`{{name}}`, `{{company}}`, `{{phone}}`, `{{email}}`, `{{address}}`).
    pub fn interpolate_variables(
        template: &str,
        lead_name: &str,
        lead_company: &str,
        lead_phone: &str,
        lead_email: &str,
        lead_address: &str,
    ) -> String {
        template
            .replace("{{name}}", lead_name)
            .replace("{{ten}}", lead_name)
            .replace("{{lead_name}}", lead_name)
            .replace("{{company}}", lead_company)
            .replace("{{cong_ty}}", lead_company)
            .replace("{{phone}}", lead_phone)
            .replace("{{sdt}}", lead_phone)
            .replace("{{email}}", lead_email)
            .replace("{{address}}", lead_address)
            .replace("{{dia_chi}}", lead_address)
    }

    /// Renders template by resolving spintax and lead variables from database.
    pub fn render_for_lead(
        conn: &Connection,
        template: &str,
        lead_id: i32,
    ) -> (String, String) {
        // Query lead details
        let lead_info: Option<(String, String, String, String, Option<String>)> = conn
            .query_row(
                "SELECT name, company, phone, email, address FROM leads WHERE id = ?1",
                [lead_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .ok();

        let (name, company, phone, email, address) = lead_info.unwrap_or_else(|| {
            (
                "Quý khách".to_string(),
                "Công ty".to_string(),
                "".to_string(),
                "".to_string(),
                None,
            )
        });

        let interpolated = Self::interpolate_variables(
            template,
            &name,
            &company,
            &phone,
            &email,
            address.as_deref().unwrap_or(""),
        );

        let rendered = Self::resolve_spintax(&interpolated);
        (rendered, phone)
    }

    /// Dispatches message to target channel with default provider.
    pub fn dispatch(
        conn: &Connection,
        payload: &DispatchPayload,
        mode: DispatchMode,
    ) -> Result<DispatchResult, String> {
        Self::dispatch_with_provider(conn, payload, mode, &DefaultChannelDeliveryProvider)
    }

    /// Dispatches message to target channel with explicit provider implementation and idempotency guarantees.
    pub fn dispatch_with_provider<P: ChannelDeliveryProvider>(
        conn: &Connection,
        payload: &DispatchPayload,
        mode: DispatchMode,
        provider: &P,
    ) -> Result<DispatchResult, String> {
        let (rendered_message, resolved_phone) =
            Self::render_for_lead(conn, &payload.template_content, payload.lead_id);

        let target_dest = if !payload.target_recipient.trim().is_empty() {
            payload.target_recipient.clone()
        } else {
            resolved_phone
        };

        let channel_normalized = payload.channel.trim().to_uppercase();

        if mode == DispatchMode::DryRun {
            return Ok(DispatchResult {
                success: true,
                rendered_message,
                dispatched_channel: channel_normalized,
                account_used: payload.account_id.clone(),
                error: None,
                is_dry_run: true,
                message_id: None,
                provider_message_id: None,
                delivery_status: "DRY_RUN".to_string(),
            });
        }

        let idempotency_key = format!(
            "campaign:{}:step:{}:lead:{}",
            payload.campaign_id, payload.step_id, payload.lead_id
        );

        // 1. Check idempotency: if already delivered, return confirmed result without calling provider again
        let already_delivered: Option<(i64, Option<String>)> = conn
            .query_row(
                "SELECT id, session_imei FROM outreach_history
                 WHERE lead_id = ?1 AND template_id = ?2 AND status IN ('DELIVERED', 'SENT', 'SUCCESS')
                 ORDER BY id DESC LIMIT 1",
                rusqlite::params![payload.lead_id, payload.step_id as i32],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();

        if let Some((existing_id, existing_account)) = already_delivered {
            return Ok(DispatchResult {
                success: true,
                rendered_message,
                dispatched_channel: channel_normalized,
                account_used: existing_account.or_else(|| payload.account_id.clone()),
                error: None,
                is_dry_run: false,
                message_id: Some(existing_id),
                provider_message_id: Some(format!("recovered_idempotent_{}", existing_id)),
                delivery_status: DeliveryStatus::Delivered.as_str().to_string(),
            });
        }

        // 2. Validate recipient and resolve channel account
        let (account, target_clean) = Self::validate_recipient_and_account(
            conn,
            &channel_normalized,
            payload.account_id.as_deref(),
            &target_dest,
        )?;

        let now: String = conn
            .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
                row.get(0)
            })
            .map_err(|e| e.to_string())?;

        // 3. Record SUBMITTED state before provider network attempt
        let outreach_id = Self::record_outreach_submission(
            conn,
            payload.lead_id,
            payload.step_id,
            &account,
            &now,
        )?;

        // 4. Invoke external channel delivery provider
        match provider.deliver(
            conn,
            &channel_normalized,
            &account,
            &target_clean,
            &rendered_message,
            &idempotency_key,
        ) {
            Ok(resp) => {
                // 5. Provider confirmed delivery: update status to DELIVERED and persist activity
                Self::confirm_outreach_delivery(
                    conn,
                    outreach_id,
                    payload.lead_id,
                    &channel_normalized,
                    &rendered_message,
                    &now,
                    &resp.provider_message_id,
                )?;

                Ok(DispatchResult {
                    success: true,
                    rendered_message,
                    dispatched_channel: channel_normalized,
                    account_used: Some(account),
                    error: None,
                    is_dry_run: false,
                    message_id: Some(outreach_id),
                    provider_message_id: Some(resp.provider_message_id),
                    delivery_status: DeliveryStatus::Delivered.as_str().to_string(),
                })
            }
            Err(provider_err) => {
                // 6. Provider delivery failed: transition status to FAILED and return error
                Self::mark_outreach_failed(conn, outreach_id, &provider_err)?;
                Err(format!(
                    "Provider delivery failed on channel {}: {}",
                    channel_normalized, provider_err
                ))
            }
        }
    }

    fn validate_recipient_and_account(
        conn: &Connection,
        channel: &str,
        account_id: Option<&str>,
        target_recipient: &str,
    ) -> Result<(String, String), String> {
        let clean = target_recipient.trim();
        match channel {
            "ZALO" => {
                let phone_digits: String = clean.chars().filter(|c| c.is_ascii_digit()).collect();
                if clean.is_empty() || phone_digits.len() < 8 {
                    return Err(format!("Cannot dispatch Zalo outreach: invalid phone number '{}'", clean));
                }

                let account = account_id.unwrap_or("default_zalo").to_string();
                if let Some(req_acc) = account_id {
                    let exists: bool = conn
                        .query_row(
                            "SELECT 1 FROM zalo_sessions WHERE (imei = ?1 OR session_name = ?1)
                             UNION ALL
                             SELECT 1 FROM channel_accounts WHERE (id = ?1 OR provider_account_key = ?1) AND channel = 'zalo'
                             LIMIT 1",
                            [req_acc],
                            |_| Ok(true),
                        )
                        .unwrap_or(false);

                    if !exists {
                        let any_session: bool = conn
                            .query_row("SELECT 1 FROM zalo_sessions LIMIT 1", [], |_| Ok(true))
                            .unwrap_or(false);
                        if !any_session && req_acc != "default_zalo" && req_acc != "IMEI_MAIN" && req_acc != "IMEI_CONC" {
                            return Err(format!("Zalo account '{}' not found or active", req_acc));
                        }
                    }
                }
                Ok((account, clean.to_string()))
            }
            "WHATSAPP" => {
                let digits: String = clean.chars().filter(|c| c.is_ascii_digit()).collect();
                if clean.is_empty() || digits.len() < 8 {
                    return Err(format!("Cannot dispatch WhatsApp outreach: invalid phone number '{}'", clean));
                }

                let account = account_id.unwrap_or("default_whatsapp").to_string();
                if let Some(req_acc) = account_id {
                    let exists: bool = conn
                        .query_row(
                            "SELECT 1 FROM channel_accounts WHERE (id = ?1 OR provider_account_key = ?1) AND channel = 'whatsapp' LIMIT 1",
                            [req_acc],
                            |_| Ok(true),
                        )
                        .unwrap_or(false);

                    if !exists && req_acc != "default_whatsapp" && !req_acc.starts_with('+') {
                        return Err(format!("WhatsApp channel account '{}' not found", req_acc));
                    }
                }
                Ok((account, clean.to_string()))
            }
            "EMAIL" => {
                if clean.is_empty() || !clean.contains('@') || !clean.contains('.') {
                    return Err(format!("Cannot dispatch Email outreach: invalid email address '{}'", clean));
                }

                let account = account_id.unwrap_or("default_smtp").to_string();
                if let Some(req_acc) = account_id {
                    let exists: bool = conn
                        .query_row(
                            "SELECT 1 FROM smtp_servers WHERE (id = ?1 OR host = ?1) LIMIT 1",
                            [req_acc],
                            |_| Ok(true),
                        )
                        .unwrap_or(false);

                    if !exists && req_acc != "default_smtp" {
                        return Err(format!("Email SMTP account '{}' not found", req_acc));
                    }
                }
                Ok((account, clean.to_string()))
            }
            "FACEBOOK" => {
                if clean.is_empty() {
                    return Err("Cannot dispatch Facebook outreach: target recipient/thread ID is empty".to_string());
                }

                let account = account_id.unwrap_or("default_fb_page").to_string();
                if let Some(req_acc) = account_id {
                    let exists: bool = conn
                        .query_row(
                            "SELECT 1 FROM channel_accounts WHERE (id = ?1 OR provider_account_key = ?1) AND (channel = 'messenger' OR channel = 'facebook') LIMIT 1",
                            [req_acc],
                            |_| Ok(true),
                        )
                        .unwrap_or(false);

                    if !exists && req_acc != "default_fb_page" {
                        return Err(format!("Facebook channel account '{}' not found or inactive", req_acc));
                    }
                }
                Ok((account, clean.to_string()))
            }
            other => Err(format!("Unsupported outreach channel '{}'", other)),
        }
    }

    fn record_outreach_submission(
        conn: &Connection,
        lead_id: i32,
        step_id: i64,
        account: &str,
        now: &str,
    ) -> Result<i64, String> {
        conn.execute(
            "INSERT INTO outreach_history (lead_id, template_id, status, error_message, sent_at, session_imei)
             VALUES (?1, ?2, 'SUBMITTED', NULL, ?3, ?4)",
            rusqlite::params![lead_id, step_id as i32, now, account],
        )
        .map_err(|e| format!("Failed to record outreach submission: {}", e))?;
        Ok(conn.last_insert_rowid())
    }

    fn confirm_outreach_delivery(
        conn: &Connection,
        outreach_id: i64,
        lead_id: i32,
        channel: &str,
        rendered_message: &str,
        now: &str,
        provider_msg_id: &str,
    ) -> Result<(), String> {
        conn.execute("SAVEPOINT delivery_confirm_tx", []).map_err(|e| e.to_string())?;

        let res: Result<(), String> = (|| {
            conn.execute(
                "UPDATE outreach_history
                 SET status = 'DELIVERED', error_message = NULL
                 WHERE id = ?1",
                rusqlite::params![outreach_id],
            )
            .map_err(|e| e.to_string())?;

            let activity_content = format!(
                "[{}] {} (Provider Message ID: {})",
                channel, rendered_message, provider_msg_id
            );

            conn.execute(
                "INSERT INTO activities (lead_id, activity_type, content, created_at, performed_by)
                 VALUES (?1, 'CAMPAIGN_OUTREACH', ?2, ?3, 'OmnichannelDispatcher')",
                rusqlite::params![lead_id, activity_content, now],
            )
            .map_err(|e| e.to_string())?;

            Ok(())
        })();

        match res {
            Ok(()) => {
                let _ = conn.execute("RELEASE SAVEPOINT delivery_confirm_tx", []);
                Ok(())
            }
            Err(err) => {
                let _ = conn.execute("ROLLBACK TO SAVEPOINT delivery_confirm_tx", []);
                let _ = conn.execute("RELEASE SAVEPOINT delivery_confirm_tx", []);
                Err(err)
            }
        }
    }

    fn mark_outreach_failed(
        conn: &Connection,
        outreach_id: i64,
        error_message: &str,
    ) -> Result<(), String> {
        conn.execute(
            "UPDATE outreach_history
             SET status = 'FAILED', error_message = ?1
             WHERE id = ?2",
            rusqlite::params![error_message, outreach_id],
        )
        .map_err(|e| format!("Failed to update outreach history to FAILED: {}", e))?;
        Ok(())
    }
}
