use crate::commands::DbState;
use crate::host::State;
use crate::models::{Campaign, CampaignStep};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepAnalytics {
    pub step_id: i64,
    pub step_order: i32,
    pub channel: String,
    #[serde(default)]
    pub retired: bool,
    pub pending: i64,
    pub sent: i64,
    pub delivered: i64,
    pub replied: i64,
    pub failed: i64,
    pub skipped: i64,
}

/// A step an edit would remove while customers still wait at it (their send is `PENDING`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemovedStepImpact {
    pub step_id: i64,
    pub step_order: i32,
    pub channel: String,
    /// Distinct customers whose pending send at this step the save would cancel.
    pub pending_leads: i64,
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

pub fn start_unified_campaign(campaign_id: i64, state: State<'_, DbState>) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    start_unified_campaign_internal(&conn, campaign_id)
}

pub fn pause_unified_campaign(campaign_id: i64, state: State<'_, DbState>) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    pause_unified_campaign_internal(&conn, campaign_id)
}

pub fn enroll_leads_into_campaign(
    campaign_id: i64,
    lead_ids: Vec<i32>,
    state: State<'_, DbState>,
) -> Result<usize, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    enroll_leads_into_campaign_internal(&conn, campaign_id, &lead_ids)
}

pub fn get_unified_campaign_analytics(
    campaign_id: i64,
    state: State<'_, DbState>,
) -> Result<CampaignAnalytics, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    get_unified_campaign_analytics_internal(&conn, campaign_id)
}

pub fn get_unified_campaign_steps(
    campaign_id: i64,
    state: State<'_, DbState>,
) -> Result<Vec<CampaignStep>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    get_unified_campaign_steps_internal(&conn, campaign_id)
}

pub fn get_all_unified_campaigns(state: State<'_, DbState>) -> Result<Vec<Campaign>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    get_all_unified_campaigns_internal(&conn)
}

pub fn get_unified_campaign_save_impact(
    campaign_id: i64,
    steps: Vec<CampaignStep>,
    state: State<'_, DbState>,
) -> Result<Vec<RemovedStepImpact>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    get_unified_campaign_save_impact_internal(&conn, campaign_id, &steps)
}

pub fn save_unified_campaign(
    campaign: Campaign,
    steps: Vec<CampaignStep>,
    confirm_remove_pending: Option<bool>,
    state: State<'_, DbState>,
) -> Result<i64, String> {
    let mut conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    save_unified_campaign_internal(
        &mut conn,
        campaign,
        steps,
        confirm_remove_pending.unwrap_or(false),
    )
}

pub fn delete_unified_campaign(campaign_id: i64, state: State<'_, DbState>) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    delete_unified_campaign_internal(&conn, campaign_id)
}

// ============================================================================
// Internal Handlers (Testable without Tauri Runtime)
// ============================================================================

pub fn start_unified_campaign_internal(conn: &Connection, campaign_id: i64) -> Result<(), String> {
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

pub fn pause_unified_campaign_internal(conn: &Connection, campaign_id: i64) -> Result<(), String> {
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
            .query_row("SELECT phone FROM leads WHERE id = ?1", [lead_id], |row| {
                row.get(0)
            })
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

    let steps = crate::db::get_campaign_steps_for_analytics(conn, campaign_id)
        .map_err(|e| format!("DB error: {}", e))?;
    let live_steps = steps.iter().filter(|(_, retired)| !*retired).count() as i64;

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

    let mut step_counts_stmt = conn
        .prepare(
            "SELECT status, COUNT(*) FROM campaign_executions
             WHERE step_id = ?1
             GROUP BY status",
        )
        .map_err(|e| format!("DB error: {}", e))?;

    let mut step_breakdown = Vec::with_capacity(steps.len());
    for (step, retired) in &steps {
        if let Some(step_id) = step.id {
            let mut s_pending = 0i64;
            let mut s_sent = 0i64;
            let mut s_delivered = 0i64;
            let mut s_replied = 0i64;
            let mut s_failed = 0i64;
            let mut s_skipped = 0i64;

            let rows = step_counts_stmt
                .query_map([step_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })
                .map_err(|e| format!("DB error: {}", e))?;
            for row in rows {
                let (status, count) = row.map_err(|e| format!("DB error: {}", e))?;
                match status.as_str() {
                    "PENDING" => s_pending = count,
                    "SENT" => s_sent = count,
                    "DELIVERED" => s_delivered = count,
                    "REPLIED" => s_replied = count,
                    "FAILED" => s_failed = count,
                    "SKIPPED" => s_skipped = count,
                    _ => {}
                }
            }

            step_breakdown.push(StepAnalytics {
                step_id,
                step_order: step.step_order,
                channel: step.channel.clone(),
                retired: *retired,
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
        total_steps: live_steps,
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

/// Real steps of campaign `campaign_id`, in `step_order` (ties by id). Read-only.
///
/// The campaign must exist: an unknown id is an error rather than an empty list, so an edit
/// screen cannot mistake "no such campaign" for "campaign with no steps" and save over it.
/// Steps are matched on `campaign_id` only, so another campaign's steps are never returned.
pub fn get_unified_campaign_steps_internal(
    conn: &Connection,
    campaign_id: i64,
) -> Result<Vec<CampaignStep>, String> {
    crate::db::get_campaign(conn, campaign_id)
        .map_err(|e| format!("DB error: {}", e))?
        .ok_or_else(|| format!("Campaign {} not found", campaign_id))?;
    crate::db::get_campaign_steps(conn, campaign_id).map_err(|e| format!("DB error: {}", e))
}

pub fn get_all_unified_campaigns_internal(conn: &Connection) -> Result<Vec<Campaign>, String> {
    crate::db::get_all_campaigns(conn).map_err(|e| format!("Failed to get campaigns: {}", e))
}

/// Cancelled execution: the customer was waiting at a step the edit removed. Kept as history.
pub(crate) const STEP_REMOVED_NOTE: &str = "Step removed while editing the campaign";

/// The live steps of `campaign_id` and, of those, the ones `submitted` leaves out.
///
/// A submitted step with a positive `id` must be one of the campaign's live steps (it is updated
/// in place); a step without one is new. A foreign, stale or repeated id is refused, so an edit
/// can never touch another campaign's step.
fn plan_step_changes(
    conn: &Connection,
    campaign_id: i64,
    submitted: &[CampaignStep],
) -> Result<Vec<CampaignStep>, String> {
    let existing =
        crate::db::get_campaign_steps(conn, campaign_id).map_err(|e| format!("DB error: {}", e))?;
    let mut seen: Vec<i64> = Vec::new();
    for step in submitted {
        let Some(id) = step.id.filter(|id| *id > 0) else {
            continue;
        };
        if !existing.iter().any(|e| e.id == Some(id)) {
            return Err(format!(
                "Invalid step id {}: it is not a step of campaign {}",
                id, campaign_id
            ));
        }
        if seen.contains(&id) {
            return Err(format!("Invalid steps: step id {} appears twice", id));
        }
        seen.push(id);
    }
    Ok(existing
        .into_iter()
        .filter(|e| e.id.is_some_and(|id| !seen.contains(&id)))
        .collect())
}

fn pending_leads_at_step(conn: &Connection, step_id: i64) -> Result<i64, String> {
    conn.query_row(
        "SELECT COUNT(DISTINCT lead_id) FROM campaign_executions
         WHERE step_id = ?1 AND status = 'PENDING'",
        [step_id],
        |row| row.get(0),
    )
    .map_err(|e| format!("DB error: {}", e))
}

/// What saving `steps` over campaign `campaign_id` would cost: the steps it removes that still
/// have customers waiting, with how many. Read-only; an empty list means the save loses nothing
/// in flight. An unknown campaign is an error.
pub fn get_unified_campaign_save_impact_internal(
    conn: &Connection,
    campaign_id: i64,
    steps: &[CampaignStep],
) -> Result<Vec<RemovedStepImpact>, String> {
    crate::db::get_campaign(conn, campaign_id)
        .map_err(|e| format!("DB error: {}", e))?
        .ok_or_else(|| format!("Campaign {} not found", campaign_id))?;
    let mut impact = Vec::new();
    for step in plan_step_changes(conn, campaign_id, steps)? {
        let Some(step_id) = step.id else { continue };
        let pending_leads = pending_leads_at_step(conn, step_id)?;
        if pending_leads > 0 {
            impact.push(RemovedStepImpact {
                step_id,
                step_order: step.step_order,
                channel: step.channel,
                pending_leads,
            });
        }
    }
    Ok(impact)
}

/// Saves `campaign` and its `steps` in one transaction.
///
/// Editing an existing campaign keeps the steps' ids: a submitted step with an id is updated in
/// place, one without is inserted, and only a step the edit leaves out is removed. Enrolments and
/// history at the kept steps are untouched. For a removed step, customers still `PENDING` there
/// are set to `CANCELLED` (they are not deleted), and the step row stays, retired, when any
/// execution points at it, so the sends already made are not cascade-deleted with it.
pub fn save_unified_campaign_internal(
    conn: &mut Connection,
    campaign: Campaign,
    steps: Vec<CampaignStep>,
    confirm_remove_pending: bool,
) -> Result<i64, String> {
    let tx = conn
        .transaction()
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;

    let (campaign_id, removed) = match campaign.id {
        Some(id) if id > 0 => {
            crate::db::get_campaign(&tx, id)
                .map_err(|e| format!("DB error: {}", e))?
                .ok_or_else(|| format!("Campaign {} not found", id))?;
            let removed = plan_step_changes(&tx, id, &steps)?;
            if !confirm_remove_pending {
                let mut impacted: Vec<String> = Vec::new();
                for step in &removed {
                    let Some(step_id) = step.id else { continue };
                    let pending = pending_leads_at_step(&tx, step_id)?;
                    if pending > 0 {
                        impacted.push(format!(
                            "step {} (id {}): {} pending lead(s)",
                            step.step_order, step_id, pending
                        ));
                    }
                }
                if !impacted.is_empty() {
                    return Err(format!(
                        "Confirmation required: removing step(s) with pending leads ({}); set confirm_remove_pending=true to proceed",
                        impacted.join(", ")
                    ));
                }
            }
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
            (id, removed)
        }
        _ => (
            crate::db::create_campaign(
                &tx,
                &campaign.name,
                campaign.description.as_deref(),
                &campaign.channel_type,
                &campaign.status,
                campaign.audience_filter_json.as_deref(),
                campaign.schedule_config_json.as_deref(),
            )
            .map_err(|e| format!("Failed to create campaign: {}", e))?,
            Vec::new(),
        ),
    };
    let editing = campaign.id.is_some_and(|id| id > 0);

    for step in steps {
        match step.id.filter(|id| editing && *id > 0) {
            Some(step_id) => crate::db::update_campaign_step(
                &tx,
                step_id,
                step.step_order,
                &step.channel,
                step.delay_minutes,
                &step.condition_type,
                &step.template_content,
                step.ai_prompt_override.as_deref(),
                step.template_id,
            )
            .map_err(|e| format!("Failed to update campaign step: {}", e))?,
            None => {
                crate::db::create_campaign_step(
                    &tx,
                    campaign_id,
                    step.step_order,
                    &step.channel,
                    step.delay_minutes,
                    &step.condition_type,
                    &step.template_content,
                    step.ai_prompt_override.as_deref(),
                    step.template_id,
                )
                .map_err(|e| format!("Failed to create campaign step: {}", e))?;
            }
        }
    }

    for step in removed {
        let Some(step_id) = step.id else { continue };
        tx.execute(
            "UPDATE campaign_executions
             SET status = 'CANCELLED', error_message = ?2,
                 executed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
             WHERE step_id = ?1 AND status = 'PENDING'",
            rusqlite::params![step_id, STEP_REMOVED_NOTE],
        )
        .map_err(|e| format!("Failed to cancel pending sends of a removed step: {}", e))?;
        let has_history: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM campaign_executions WHERE step_id = ?1)",
                [step_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("DB error: {}", e))?;
        if has_history {
            tx.execute(
                "UPDATE campaign_steps
                 SET retired_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE id = ?1",
                [step_id],
            )
            .map_err(|e| format!("Failed to retire a removed step: {}", e))?;
        } else {
            tx.execute("DELETE FROM campaign_steps WHERE id = ?1", [step_id])
                .map_err(|e| format!("Failed to remove a campaign step: {}", e))?;
        }
    }

    tx.commit()
        .map_err(|e| format!("Failed to commit transaction: {}", e))?;

    Ok(campaign_id)
}

pub fn delete_unified_campaign_internal(conn: &Connection, campaign_id: i64) -> Result<(), String> {
    crate::db::delete_campaign(conn, campaign_id)
        .map_err(|e| crate::commands::db_write_error(e, "campaign", campaign_id))
}
