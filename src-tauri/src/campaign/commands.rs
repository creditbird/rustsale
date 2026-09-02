use crate::commands::DbState;
use crate::models::{Campaign, CampaignStep};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepAnalytics {
    pub step_id: i64,
    pub step_order: i32,
    pub channel: String,
    pub pending: i64,
    pub sent: i64,
    pub delivered: i64,
    pub replied: i64,
    pub failed: i64,
    pub skipped: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignAnalytics {
    pub campaign_id: i64,
    pub campaign_name: String,
    pub campaign_status: String,
    pub total_enrolled: i64,
    pub total_steps: i64,
    pub total_executions: i64,
    pub pending_count: i64,
    pub sent_count: i64,
    pub delivered_count: i64,
    pub replied_count: i64,
    pub failed_count: i64,
    pub skipped_count: i64,
    pub converted_count: i64,
    pub response_rate: f64,
    pub conversion_rate: f64,
    pub step_breakdown: Vec<StepAnalytics>,
}

#[tauri::command]
pub fn start_unified_campaign(
    campaign_id: i64,
    state: tauri::State<'_, DbState>,
) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    start_unified_campaign_internal(&conn, campaign_id)
}

#[tauri::command]
pub fn pause_unified_campaign(
    campaign_id: i64,
    state: tauri::State<'_, DbState>,
) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    pause_unified_campaign_internal(&conn, campaign_id)
}

#[tauri::command]
pub fn enroll_leads_into_campaign(
    campaign_id: i64,
    lead_ids: Vec<i32>,
    state: tauri::State<'_, DbState>,
) -> Result<usize, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    enroll_leads_into_campaign_internal(&conn, campaign_id, &lead_ids)
}

#[tauri::command]
pub fn get_unified_campaign_analytics(
    campaign_id: i64,
    state: tauri::State<'_, DbState>,
) -> Result<CampaignAnalytics, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    get_unified_campaign_analytics_internal(&conn, campaign_id)
}

#[tauri::command]
pub fn get_all_unified_campaigns(
    state: tauri::State<'_, DbState>,
) -> Result<Vec<Campaign>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    get_all_unified_campaigns_internal(&conn)
}

#[tauri::command]
pub fn save_unified_campaign(
    campaign: Campaign,
    steps: Vec<CampaignStep>,
    state: tauri::State<'_, DbState>,
) -> Result<i64, String> {
    let mut conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    save_unified_campaign_internal(&mut conn, campaign, steps)
}

#[tauri::command]
pub fn delete_unified_campaign(
    campaign_id: i64,
    state: tauri::State<'_, DbState>,
) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    delete_unified_campaign_internal(&conn, campaign_id)
}

// ============================================================================
// Internal Handlers (Testable without Tauri Runtime)
// ============================================================================

pub fn start_unified_campaign_internal(
    conn: &Connection,
    campaign_id: i64,
) -> Result<(), String> {
    let campaign = crate::db::get_campaign(conn, campaign_id)
        .map_err(|e| format!("DB error: {}", e))?
        .ok_or_else(|| format!("Campaign {} not found", campaign_id))?;

    crate::db::update_campaign(
        conn,
        campaign_id,
        &campaign.name,
        campaign.description.as_deref(),
        &campaign.channel_type,
        "ACTIVE",
        campaign.audience_filter_json.as_deref(),
        campaign.schedule_config_json.as_deref(),
    )
    .map_err(|e| format!("Failed to activate campaign: {}", e))
}

pub fn pause_unified_campaign_internal(
    conn: &Connection,
    campaign_id: i64,
) -> Result<(), String> {
    let campaign = crate::db::get_campaign(conn, campaign_id)
        .map_err(|e| format!("DB error: {}", e))?
        .ok_or_else(|| format!("Campaign {} not found", campaign_id))?;

    crate::db::update_campaign(
        conn,
        campaign_id,
        &campaign.name,
        campaign.description.as_deref(),
        &campaign.channel_type,
        "PAUSED",
        campaign.audience_filter_json.as_deref(),
        campaign.schedule_config_json.as_deref(),
    )
    .map_err(|e| format!("Failed to pause campaign: {}", e))
}

pub fn enroll_leads_into_campaign_internal(
    conn: &Connection,
    campaign_id: i64,
    lead_ids: &[i32],
) -> Result<usize, String> {
    if lead_ids.is_empty() {
        return Ok(0);
    }

    let steps = crate::db::get_campaign_steps(conn, campaign_id)
        .map_err(|e| format!("Failed to get campaign steps: {}", e))?;

    if steps.is_empty() {
        return Err(format!(
            "Campaign {} has no steps defined. Cannot enroll leads.",
            campaign_id
        ));
    }

    let first_step = &steps[0];
    let first_step_id = first_step
        .id
        .ok_or_else(|| "First step is missing an ID".to_string())?;

    let now_iso: String = conn
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })
        .map_err(|e| e.to_string())?;

    let scheduled_iso: String = if first_step.delay_minutes > 0 {
        conn.query_row(
            "SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now', ?1)",
            [format!("+{} minutes", first_step.delay_minutes)],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| now_iso.clone())
    } else {
        now_iso.clone()
    };

    let mut enrolled_count = 0;

    for &lead_id in lead_ids {
        // Fetch phone from lead
        let phone: Option<String> = conn
            .query_row(
                "SELECT phone FROM leads WHERE id = ?1",
                [lead_id],
                |row| row.get(0),
            )
            .ok();

        // Create initial step execution
        let exec_id = crate::db::create_campaign_execution(
            conn,
            campaign_id,
            first_step_id,
            lead_id,
            phone.as_deref(),
            None,
            &scheduled_iso,
        )
        .map_err(|e| format!("Failed to create execution for lead {}: {}", lead_id, e))?;

        if exec_id > 0 {
            // Update lead journey state
            let _ = crate::db::update_lead_journey_state(
                conn,
                lead_id,
                "ENROLLED",
                Some(campaign_id),
                None,
                None,
                None,
                None,
                None,
            );
            enrolled_count += 1;
        }
    }

    Ok(enrolled_count)
}

pub fn get_unified_campaign_analytics_internal(
    conn: &Connection,
    campaign_id: i64,
) -> Result<CampaignAnalytics, String> {
    let campaign = crate::db::get_campaign(conn, campaign_id)
        .map_err(|e| format!("DB error: {}", e))?
        .ok_or_else(|| format!("Campaign {} not found", campaign_id))?;

    let steps = crate::db::get_campaign_steps(conn, campaign_id)
        .map_err(|e| format!("DB error: {}", e))?;

    let total_enrolled: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT lead_id) FROM campaign_executions WHERE campaign_id = ?1",
            [campaign_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let total_executions: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM campaign_executions WHERE campaign_id = ?1",
            [campaign_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let pending_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM campaign_executions WHERE campaign_id = ?1 AND status = 'PENDING'",
            [campaign_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let sent_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM campaign_executions WHERE campaign_id = ?1 AND status = 'SENT'",
            [campaign_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let delivered_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM campaign_executions WHERE campaign_id = ?1 AND status = 'DELIVERED'",
            [campaign_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let replied_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM campaign_executions WHERE campaign_id = ?1 AND status = 'REPLIED'",
            [campaign_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let failed_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM campaign_executions WHERE campaign_id = ?1 AND status = 'FAILED'",
            [campaign_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let skipped_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM campaign_executions WHERE campaign_id = ?1 AND status = 'SKIPPED'",
            [campaign_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let converted_count: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT ce.lead_id) FROM campaign_executions ce
             JOIN deals d ON ce.lead_id = d.lead_id
             WHERE ce.campaign_id = ?1 AND UPPER(d.stage) IN ('WON', 'CLOSED_WON', 'QUALIFIED')",
            [campaign_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let mut step_breakdown = Vec::new();
    for step in &steps {
        if let Some(step_id) = step.id {
            let s_pending: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM campaign_executions WHERE step_id = ?1 AND status = 'PENDING'",
                    [step_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            let s_sent: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM campaign_executions WHERE step_id = ?1 AND status = 'SENT'",
                    [step_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            let s_delivered: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM campaign_executions WHERE step_id = ?1 AND status = 'DELIVERED'",
                    [step_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            let s_replied: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM campaign_executions WHERE step_id = ?1 AND status = 'REPLIED'",
                    [step_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            let s_failed: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM campaign_executions WHERE step_id = ?1 AND status = 'FAILED'",
                    [step_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            let s_skipped: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM campaign_executions WHERE step_id = ?1 AND status = 'SKIPPED'",
                    [step_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            step_breakdown.push(StepAnalytics {
                step_id,
                step_order: step.step_order,
                channel: step.channel.clone(),
                pending: s_pending,
                sent: s_sent,
                delivered: s_delivered,
                replied: s_replied,
                failed: s_failed,
                skipped: s_skipped,
            });
        }
    }

    let active_outbound = sent_count + delivered_count + replied_count;
    let response_rate = if active_outbound > 0 {
        (replied_count as f64 / active_outbound as f64) * 100.0
    } else {
        0.0
    };

    let conversion_rate = if total_enrolled > 0 {
        (converted_count as f64 / total_enrolled as f64) * 100.0
    } else {
        0.0
    };

    Ok(CampaignAnalytics {
        campaign_id,
        campaign_name: campaign.name,
        campaign_status: campaign.status,
        total_enrolled,
        total_steps: steps.len() as i64,
        total_executions,
        pending_count,
        sent_count,
        delivered_count,
        replied_count,
        failed_count,
        skipped_count,
        converted_count,
        response_rate,
        conversion_rate,
        step_breakdown,
    })
}

pub fn get_all_unified_campaigns_internal(
    conn: &Connection,
) -> Result<Vec<Campaign>, String> {
    crate::db::get_all_campaigns(conn).map_err(|e| format!("Failed to get campaigns: {}", e))
}

pub fn save_unified_campaign_internal(
    conn: &mut Connection,
    campaign: Campaign,
    steps: Vec<CampaignStep>,
) -> Result<i64, String> {
    let tx = conn
        .transaction()
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;

    let campaign_id = match campaign.id {
        Some(id) if id > 0 => {
            crate::db::update_campaign(
                &tx,
                id,
                &campaign.name,
                campaign.description.as_deref(),
                &campaign.channel_type,
                &campaign.status,
                campaign.audience_filter_json.as_deref(),
                campaign.schedule_config_json.as_deref(),
            )
            .map_err(|e| format!("Failed to update campaign: {}", e))?;

            // Replace steps
            tx.execute(
                "DELETE FROM campaign_steps WHERE campaign_id = ?1",
                [id],
            )
            .map_err(|e| format!("Failed to clear old campaign steps: {}", e))?;

            id
        }
        _ => crate::db::create_campaign(
            &tx,
            &campaign.name,
            campaign.description.as_deref(),
            &campaign.channel_type,
            &campaign.status,
            campaign.audience_filter_json.as_deref(),
            campaign.schedule_config_json.as_deref(),
        )
        .map_err(|e| format!("Failed to create campaign: {}", e))?,
    };

    for step in steps {
        crate::db::create_campaign_step(
            &tx,
            campaign_id,
            step.step_order,
            &step.channel,
            step.delay_minutes,
            &step.condition_type,
            &step.template_content,
            step.ai_prompt_override.as_deref(),
        )
        .map_err(|e| format!("Failed to create campaign step: {}", e))?;
    }

    tx.commit()
        .map_err(|e| format!("Failed to commit transaction: {}", e))?;

    Ok(campaign_id)
}

pub fn delete_unified_campaign_internal(
    conn: &Connection,
    campaign_id: i64,
) -> Result<(), String> {
    crate::db::delete_campaign(conn, campaign_id)
        .map_err(|e| format!("Failed to delete campaign: {}", e))
}
