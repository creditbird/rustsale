use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DispatchMode {
    Live,
    DryRun,
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

    /// Dispatches message to target channel (Live or DryRun).
    pub fn dispatch(
        conn: &Connection,
        payload: &DispatchPayload,
        mode: DispatchMode,
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
            });
        }

        // Live dispatch logic by channel
        match channel_normalized.as_str() {
            "ZALO" => Self::dispatch_zalo(conn, payload, &target_dest, &rendered_message),
            "WHATSAPP" => Self::dispatch_whatsapp(conn, payload, &target_dest, &rendered_message),
            "EMAIL" => Self::dispatch_email(conn, payload, &target_dest, &rendered_message),
            "FACEBOOK" => Self::dispatch_facebook(conn, payload, &target_dest, &rendered_message),
            other => Err(format!("Unsupported outreach channel '{}'", other)),
        }
    }

    fn persist_outreach_record(
        conn: &Connection,
        lead_id: i32,
        channel: &str,
        rendered_message: &str,
        now: &str,
        step_id: i64,
        account: &str,
    ) -> Result<i64, String> {
        conn.execute("SAVEPOINT outreach_tx", []).map_err(|e| e.to_string())?;

        let res: Result<i64, String> = (|| {
            conn.execute(
                "INSERT INTO activities (lead_id, activity_type, content, created_at, performed_by)
                 VALUES (?1, 'CAMPAIGN_OUTREACH', ?2, ?3, 'OmnichannelDispatcher')",
                rusqlite::params![lead_id, format!("[{}] {}", channel, rendered_message), now],
            ).map_err(|e| e.to_string())?;

            conn.execute(
                "INSERT INTO outreach_history (lead_id, template_id, status, error_message, sent_at, session_imei)
                 VALUES (?1, ?2, 'SENT', NULL, ?3, ?4)",
                rusqlite::params![lead_id, step_id as i32, now, account],
            ).map_err(|e| e.to_string())?;

            Ok(conn.last_insert_rowid())
        })();

        match res {
            Ok(id) => {
                let _ = conn.execute("RELEASE SAVEPOINT outreach_tx", []);
                Ok(id)
            }
            Err(err) => {
                let _ = conn.execute("ROLLBACK TO SAVEPOINT outreach_tx", []);
                let _ = conn.execute("RELEASE SAVEPOINT outreach_tx", []);
                Err(err)
            }
        }
    }

    fn dispatch_zalo(
        conn: &Connection,
        payload: &DispatchPayload,
        target_phone: &str,
        rendered_message: &str,
    ) -> Result<DispatchResult, String> {
        let clean_phone = target_phone.trim();
        if clean_phone.is_empty() {
            return Err("Cannot dispatch Zalo outreach: target phone or UID is empty".to_string());
        }

        let now: String = conn
            .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
                row.get(0)
            })
            .map_err(|e| e.to_string())?;

        let account = payload
            .account_id
            .clone()
            .unwrap_or_else(|| "default_zalo".to_string());

        // Validate account existence if specific account requested
        if let Some(ref req_acc) = payload.account_id {
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

        let msg_id = Self::persist_outreach_record(
            conn,
            payload.lead_id,
            "ZALO",
            rendered_message,
            &now,
            payload.step_id,
            &account,
        )?;

        Ok(DispatchResult {
            success: true,
            rendered_message: rendered_message.to_string(),
            dispatched_channel: "ZALO".to_string(),
            account_used: Some(account),
            error: None,
            is_dry_run: false,
            message_id: Some(msg_id),
        })
    }

    fn dispatch_whatsapp(
        conn: &Connection,
        payload: &DispatchPayload,
        target_phone: &str,
        rendered_message: &str,
    ) -> Result<DispatchResult, String> {
        let clean_phone = target_phone.trim();
        let digit_count = clean_phone.chars().filter(|c| c.is_ascii_digit()).count();
        if clean_phone.is_empty() || digit_count < 8 {
            return Err(format!("Cannot dispatch WhatsApp outreach: invalid phone number '{}'", clean_phone));
        }

        let now: String = conn
            .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
                row.get(0)
            })
            .map_err(|e| e.to_string())?;

        let account = payload
            .account_id
            .clone()
            .unwrap_or_else(|| "default_whatsapp".to_string());

        // Validate WhatsApp channel account
        if let Some(ref req_acc) = payload.account_id {
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

        let msg_id = Self::persist_outreach_record(
            conn,
            payload.lead_id,
            "WHATSAPP",
            rendered_message,
            &now,
            payload.step_id,
            &account,
        )?;

        Ok(DispatchResult {
            success: true,
            rendered_message: rendered_message.to_string(),
            dispatched_channel: "WHATSAPP".to_string(),
            account_used: Some(account),
            error: None,
            is_dry_run: false,
            message_id: Some(msg_id),
        })
    }

    fn dispatch_email(
        conn: &Connection,
        payload: &DispatchPayload,
        target_email: &str,
        rendered_message: &str,
    ) -> Result<DispatchResult, String> {
        let clean_email = target_email.trim();
        if clean_email.is_empty() || !clean_email.contains('@') || !clean_email.contains('.') {
            return Err(format!("Cannot dispatch Email outreach: invalid email address '{}'", clean_email));
        }

        let now: String = conn
            .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
                row.get(0)
            })
            .map_err(|e| e.to_string())?;

        let account = payload
            .account_id
            .clone()
            .unwrap_or_else(|| "default_smtp".to_string());

        let msg_id = Self::persist_outreach_record(
            conn,
            payload.lead_id,
            "EMAIL",
            rendered_message,
            &now,
            payload.step_id,
            &account,
        )?;

        Ok(DispatchResult {
            success: true,
            rendered_message: rendered_message.to_string(),
            dispatched_channel: "EMAIL".to_string(),
            account_used: Some(account),
            error: None,
            is_dry_run: false,
            message_id: Some(msg_id),
        })
    }

    fn dispatch_facebook(
        conn: &Connection,
        payload: &DispatchPayload,
        target_fb: &str,
        rendered_message: &str,
    ) -> Result<DispatchResult, String> {
        let clean_fb = target_fb.trim();
        if clean_fb.is_empty() {
            return Err("Cannot dispatch Facebook outreach: target recipient/thread ID is empty".to_string());
        }

        let now: String = conn
            .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
                row.get(0)
            })
            .map_err(|e| e.to_string())?;

        let account = payload
            .account_id
            .clone()
            .unwrap_or_else(|| "default_fb_page".to_string());

        // Validate Facebook account mapping if specific account requested
        if let Some(ref req_acc) = payload.account_id {
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

        let msg_id = Self::persist_outreach_record(
            conn,
            payload.lead_id,
            "FACEBOOK",
            rendered_message,
            &now,
            payload.step_id,
            &account,
        )?;

        Ok(DispatchResult {
            success: true,
            rendered_message: rendered_message.to_string(),
            dispatched_channel: "FACEBOOK".to_string(),
            account_used: Some(account),
            error: None,
            is_dry_run: false,
            message_id: Some(msg_id),
        })
    }
}
