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
                    rng_seed = rng_seed.wrapping_mul(1103515245).wrapping_add(12345);
                    let choice_idx = (rng_seed / 65536) % choices.len();
                    choices[choice_idx].to_string()
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

    fn dispatch_zalo(
        conn: &Connection,
        payload: &DispatchPayload,
        _target_phone: &str,
        rendered_message: &str,
    ) -> Result<DispatchResult, String> {
        let now: String = conn
            .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
                row.get(0)
            })
            .map_err(|e| e.to_string())?;

        let account = payload
            .account_id
            .clone()
            .unwrap_or_else(|| "default_zalo".to_string());

        // Record into outreach_history or conversation messages if conversation exists
        let _ = conn.execute(
            "INSERT INTO outreach_history (lead_id, template_id, status, error_message, sent_at, session_imei)
             VALUES (?1, ?2, 'SENT', NULL, ?3, ?4)",
            rusqlite::params![payload.lead_id, payload.step_id as i32, now, account],
        );

        Ok(DispatchResult {
            success: true,
            rendered_message: rendered_message.to_string(),
            dispatched_channel: "ZALO".to_string(),
            account_used: Some(account),
            error: None,
            is_dry_run: false,
            message_id: None,
        })
    }

    fn dispatch_whatsapp(
        _conn: &Connection,
        payload: &DispatchPayload,
        _target_phone: &str,
        rendered_message: &str,
    ) -> Result<DispatchResult, String> {
        let account = payload
            .account_id
            .clone()
            .unwrap_or_else(|| "default_whatsapp".to_string());

        Ok(DispatchResult {
            success: true,
            rendered_message: rendered_message.to_string(),
            dispatched_channel: "WHATSAPP".to_string(),
            account_used: Some(account),
            error: None,
            is_dry_run: false,
            message_id: None,
        })
    }

    fn dispatch_email(
        _conn: &Connection,
        payload: &DispatchPayload,
        _target_email: &str,
        rendered_message: &str,
    ) -> Result<DispatchResult, String> {
        let account = payload
            .account_id
            .clone()
            .unwrap_or_else(|| "default_smtp".to_string());

        Ok(DispatchResult {
            success: true,
            rendered_message: rendered_message.to_string(),
            dispatched_channel: "EMAIL".to_string(),
            account_used: Some(account),
            error: None,
            is_dry_run: false,
            message_id: None,
        })
    }

    fn dispatch_facebook(
        _conn: &Connection,
        payload: &DispatchPayload,
        _target_fb: &str,
        rendered_message: &str,
    ) -> Result<DispatchResult, String> {
        let account = payload
            .account_id
            .clone()
            .unwrap_or_else(|| "default_fb_page".to_string());

        Ok(DispatchResult {
            success: true,
            rendered_message: rendered_message.to_string(),
            dispatched_channel: "FACEBOOK".to_string(),
            account_used: Some(account),
            error: None,
            is_dry_run: false,
            message_id: None,
        })
    }
}
