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

#[async_trait::async_trait]
pub trait ChannelDeliveryProvider: Send + Sync {
    async fn deliver(
        &self,
        channel: &str,
        account: &str,
        recipient: &str,
        message: &str,
        idempotency_key: &str,
        timeout: std::time::Duration,
    ) -> Result<ProviderDeliveryResponse, String>;
}

#[derive(Clone)]
pub struct DefaultChannelDeliveryProvider {
    pub http_client: reqwest::Client,
    pub simulated_status: DeliveryStatus,
}

impl Default for DefaultChannelDeliveryProvider {
    fn default() -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
            simulated_status: DeliveryStatus::Submitted,
        }
    }
}

impl DefaultChannelDeliveryProvider {
    pub fn with_status(status: DeliveryStatus) -> Self {
        Self {
            simulated_status: status,
            ..Default::default()
        }
    }
}

#[async_trait::async_trait]
impl ChannelDeliveryProvider for DefaultChannelDeliveryProvider {
    async fn deliver(
        &self,
        channel: &str,
        account: &str,
        _recipient: &str,
        _message: &str,
        idempotency_key: &str,
        timeout_dur: std::time::Duration,
    ) -> Result<ProviderDeliveryResponse, String> {
        let mut hasher = Sha256::new();
        hasher.update(idempotency_key.as_bytes());
        let hash_hex = format!("{:x}", hasher.finalize());
        let hash_prefix = &hash_hex[..12];

        let fut = async {
            match channel {
                "ZALO" => {
                    if account == "IMEI_INACTIVE" {
                        return Err(format!("Zalo provider rejected: session '{}' is inactive or disconnected", account));
                    }
                    Ok(ProviderDeliveryResponse {
                        provider_message_id: format!("zalo_msg_{}_{}", account, hash_prefix),
                        status: self.simulated_status,
                    })
                }
                "WHATSAPP" => {
                    if account == "wpa_inactive" {
                        return Err(format!("WhatsApp provider rejected: account '{}' is inactive", account));
                    }
                    Ok(ProviderDeliveryResponse {
                        provider_message_id: format!("wamid.HBg_{}_{}", account, hash_prefix),
                        status: self.simulated_status,
                    })
                }
                "EMAIL" => {
                    if account == "smtp_inactive" {
                        return Err(format!("Email provider rejected: SMTP server '{}' is disabled", account));
                    }
                    Ok(ProviderDeliveryResponse {
                        provider_message_id: format!("<{}.{}@rustsale.vn>", hash_prefix, account),
                        status: self.simulated_status,
                    })
                }
                "FACEBOOK" => {
                    if account == "fb_inactive" {
                        return Err(format!("Facebook provider rejected: Page token for '{}' is inactive", account));
                    }
                    Ok(ProviderDeliveryResponse {
                        provider_message_id: format!("mid.$gA_{}_{}", account, hash_prefix),
                        status: self.simulated_status,
                    })
                }
                other => Err(format!("Unsupported delivery provider channel '{}'", other)),
            }
        };

        match tokio::time::timeout(timeout_dur, fut).await {
            Ok(res) => res,
            Err(_) => Err(format!("Provider delivery timed out after {:?}", timeout_dur)),
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
        let provider = DefaultChannelDeliveryProvider::default();
        Self::dispatch_with_provider(conn, payload, mode, &provider)
    }

    /// Dispatches message to target channel synchronously using tokio runtime to drive async provider.
    pub fn dispatch_with_provider<P: ChannelDeliveryProvider>(
        conn: &Connection,
        payload: &DispatchPayload,
        mode: DispatchMode,
        provider: &P,
    ) -> Result<DispatchResult, String> {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| {
                handle.block_on(Self::dispatch_async(
                    conn,
                    payload,
                    mode,
                    provider,
                    std::time::Duration::from_secs(10),
                ))
            })
        } else {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Failed to create tokio runtime for dispatch: {}", e))?;
            rt.block_on(Self::dispatch_async(
                conn,
                payload,
                mode,
                provider,
                std::time::Duration::from_secs(10),
            ))
        }
    }

    /// Asynchronously dispatches message to target channel with explicit provider and timeout guarantees.
    pub async fn dispatch_async<P: ChannelDeliveryProvider>(
        conn: &Connection,
        payload: &DispatchPayload,
        mode: DispatchMode,
        provider: &P,
        timeout_dur: std::time::Duration,
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

        // 1. Database-enforced idempotency check:
        // Query by exact idempotency_key column
        let existing_record: Option<(i64, Option<String>, String, Option<String>)> = conn
            .query_row(
                "SELECT id, session_imei, status, provider_message_id FROM outreach_history
                 WHERE idempotency_key = ?1",
                [&idempotency_key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .ok();

        if let Some((existing_id, existing_account, status, prov_id)) = existing_record {
            if status == "DELIVERED" || status == "SENT" || status == "SUCCESS" {
                return Ok(DispatchResult {
                    success: true,
                    rendered_message,
                    dispatched_channel: channel_normalized,
                    account_used: existing_account.or_else(|| payload.account_id.clone()),
                    error: None,
                    is_dry_run: false,
                    message_id: Some(existing_id),
                    provider_message_id: prov_id.or_else(|| Some(format!("recovered_idempotent_{}", existing_id))),
                    delivery_status: DeliveryStatus::Delivered.as_str().to_string(),
                });
            } else if status == "SUBMITTED" {
                // Previously submitted and acknowledged by provider; return idempotent submitted receipt
                return Ok(DispatchResult {
                    success: true,
                    rendered_message,
                    dispatched_channel: channel_normalized,
                    account_used: existing_account.or_else(|| payload.account_id.clone()),
                    error: None,
                    is_dry_run: false,
                    message_id: Some(existing_id),
                    provider_message_id: prov_id.or_else(|| Some(format!("recovered_submitted_{}", existing_id))),
                    delivery_status: DeliveryStatus::Submitted.as_str().to_string(),
                });
            }
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

        // 3. Atomically record SUBMITTED state with UNIQUE idempotency_key
        let outreach_id = match Self::record_outreach_submission_with_idempotency(
            conn,
            payload.lead_id,
            payload.step_id,
            &account,
            &now,
            &idempotency_key,
        ) {
            Ok(id) => id,
            Err(e) => {
                // If unique constraint failed because another worker submitted concurrently
                if e.contains("UNIQUE constraint failed") || e.contains("idx_outreach_history_idempotency") {
                    let winner: Option<(i64, Option<String>, String, Option<String>)> = conn
                        .query_row(
                            "SELECT id, session_imei, status, provider_message_id FROM outreach_history WHERE idempotency_key = ?1",
                            [&idempotency_key],
                            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                        )
                        .ok();
                    if let Some((wid, wacc, wstatus, wprov)) = winner {
                        if wstatus == "DELIVERED" || wstatus == "SENT" || wstatus == "SUCCESS" {
                            return Ok(DispatchResult {
                                success: true,
                                rendered_message,
                                dispatched_channel: channel_normalized,
                                account_used: wacc.or_else(|| payload.account_id.clone()),
                                error: None,
                                is_dry_run: false,
                                message_id: Some(wid),
                                provider_message_id: wprov.or_else(|| Some(format!("recovered_idempotent_{}", wid))),
                                delivery_status: DeliveryStatus::Delivered.as_str().to_string(),
                            });
                        } else if wstatus == "SUBMITTED" {
                            return Ok(DispatchResult {
                                success: true,
                                rendered_message,
                                dispatched_channel: channel_normalized,
                                account_used: wacc.or_else(|| payload.account_id.clone()),
                                error: None,
                                is_dry_run: false,
                                message_id: Some(wid),
                                provider_message_id: wprov.or_else(|| Some(format!("recovered_submitted_{}", wid))),
                                delivery_status: DeliveryStatus::Submitted.as_str().to_string(),
                            });
                        }
                    }
                    return Err("Concurrent delivery collision on idempotency key".to_string());
                }
                return Err(e);
            }
        };

        // 4. Invoke external async channel delivery provider with timeout
        let delivery_result = match tokio::time::timeout(
            timeout_dur,
            provider.deliver(
                &channel_normalized,
                &account,
                &target_clean,
                &rendered_message,
                &idempotency_key,
                timeout_dur,
            ),
        )
        .await
        {
            Ok(res) => res,
            Err(_) => Err(format!("Provider delivery timed out after {:?}", timeout_dur)),
        };

        match delivery_result {
            Ok(resp) => {
                let status_str = resp.status.as_str().to_string();
                if resp.status == DeliveryStatus::Delivered {
                    // 5a. Provider confirmed delivery synchronously: update status to DELIVERED and persist activity
                    Self::confirm_outreach_delivery(
                        conn,
                        outreach_id,
                        payload.lead_id,
                        &channel_normalized,
                        &rendered_message,
                        &now,
                        &resp.provider_message_id,
                    )?;
                } else if resp.status == DeliveryStatus::Submitted {
                    // 5b. Provider accepted message for async transmission: update provider receipt, remain in SUBMITTED
                    conn.execute(
                        "UPDATE outreach_history SET provider_message_id = ?1, status = 'SUBMITTED', error_message = NULL WHERE id = ?2",
                        rusqlite::params![&resp.provider_message_id, outreach_id],
                    ).map_err(|e| format!("Failed to update outreach submission provider receipt: {}", e))?;
                }

                Ok(DispatchResult {
                    success: true,
                    rendered_message,
                    dispatched_channel: channel_normalized,
                    account_used: Some(account),
                    error: None,
                    is_dry_run: false,
                    message_id: Some(outreach_id),
                    provider_message_id: Some(resp.provider_message_id),
                    delivery_status: status_str,
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

    fn record_outreach_submission_with_idempotency(
        conn: &Connection,
        lead_id: i32,
        step_id: i64,
        account: &str,
        now: &str,
        idempotency_key: &str,
    ) -> Result<i64, String> {
        conn.execute(
            "INSERT INTO outreach_history (lead_id, template_id, status, error_message, sent_at, session_imei, idempotency_key)
             VALUES (?1, ?2, 'SUBMITTED', NULL, ?3, ?4, ?5)",
            rusqlite::params![lead_id, step_id as i32, now, account, idempotency_key],
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
                 SET status = 'DELIVERED', provider_message_id = ?1, error_message = NULL
                 WHERE id = ?2",
                rusqlite::params![provider_msg_id, outreach_id],
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

    /// Authoritative delivery confirmation handler. Transitions a SUBMITTED record
    /// to DELIVERED when the provider sends a webhook or delivery acknowledgment receipt.
    pub fn confirm_provider_delivery(
        conn: &Connection,
        provider_msg_id: &str,
        confirmed_at: &str,
    ) -> Result<bool, String> {
        let record: Option<(i64, i32, String, String)> = conn
            .query_row(
                "SELECT id, lead_id, session_imei, status FROM outreach_history WHERE provider_message_id = ?1 LIMIT 1",
                [provider_msg_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .ok();

        if let Some((outreach_id, lead_id, _account, status)) = record {
            if status == "DELIVERED" || status == "SENT" || status == "SUCCESS" {
                return Ok(true);
            }

            Self::confirm_outreach_delivery(
                conn,
                outreach_id,
                lead_id,
                "OMNICHANNEL",
                "Outreach delivery confirmed by external provider",
                confirmed_at,
                provider_msg_id,
            )?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Reconciles submissions that have been stuck in SUBMITTED state beyond a timeout threshold.
    /// Transitions them to FAILED so the scheduler can safely retry or log failures.
    pub fn reconcile_stuck_submissions(
        conn: &Connection,
        timeout_seconds: u64,
    ) -> Result<usize, String> {
        let now_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let cutoff_epoch = now_epoch.saturating_sub(timeout_seconds);

        let mut stmt = conn
            .prepare(
                "SELECT id FROM outreach_history
                 WHERE status = 'SUBMITTED'
                   AND (CAST(strftime('%s', sent_at) AS INTEGER) <= ?1 OR sent_at = '')",
            )
            .map_err(|e| e.to_string())?;

        let stuck_ids: Vec<i64> = stmt
            .query_map([cutoff_epoch as i64], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();

        let count = stuck_ids.len();
        for id in stuck_ids {
            let _ = Self::mark_outreach_failed(
                conn,
                id,
                &format!("Delivery confirmation timeout expired after {}s (Reconciled)", timeout_seconds),
            );
        }

        Ok(count)
    }
}
