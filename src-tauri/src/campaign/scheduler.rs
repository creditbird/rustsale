use crate::campaign::dispatcher::{DispatchMode, DispatchPayload, OmnichannelDispatcher};
use crate::campaign::evaluator::{ConditionResult, StepConditionEvaluator};
use crate::campaign::throttling::{AccountRotator, AntiBanThrottler};
use crate::events::{AppEvent, SharedEventBus};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchedulerProcessSummary {
    pub total_due_found: usize,
    pub sent_count: usize,
    pub skipped_count: usize,
    pub throttled_count: usize,
    pub failed_count: usize,
    pub next_steps_scheduled: usize,
}

#[derive(Clone)]
pub struct CampaignScheduler {
    pub throttler: Arc<AntiBanThrottler>,
    pub rotator: Arc<AccountRotator>,
    pub event_bus: Option<SharedEventBus>,
}

impl CampaignScheduler {
    pub fn new(
        throttler: Arc<AntiBanThrottler>,
        rotator: Arc<AccountRotator>,
        event_bus: Option<SharedEventBus>,
    ) -> Self {
        Self {
            throttler,
            rotator,
            event_bus,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(
            Arc::new(AntiBanThrottler::with_default_config()),
            Arc::new(AccountRotator::with_round_robin()),
            None,
        )
    }

    /// Processes due campaign executions synchronously in a single cycle.
    pub fn process_due_executions(
        &self,
        conn: &Connection,
        mode: DispatchMode,
        limit: usize,
    ) -> Result<SchedulerProcessSummary, String> {
        let now_iso: String = conn
            .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
                row.get(0)
            })
            .map_err(|e| format!("Failed to get current timestamp: {}", e))?;

        let due_executions = crate::db::get_due_campaign_executions(conn, &now_iso, limit)
            .map_err(|e| format!("Failed to query due campaign executions: {}", e))?;

        let mut summary = SchedulerProcessSummary {
            total_due_found: due_executions.len(),
            ..Default::default()
        };

        for execution in due_executions {
            let exec_id = match execution.id {
                Some(id) => id,
                None => continue,
            };

            // 1. Verify campaign status is ACTIVE
            let campaign_status: Option<String> = conn
                .query_row(
                    "SELECT status FROM campaigns WHERE id = ?1",
                    [execution.campaign_id],
                    |row| row.get(0),
                )
                .ok();

            if campaign_status.as_deref() != Some("ACTIVE") {
                // If campaign is not ACTIVE (e.g. PAUSED or DRAFT), skip execution for now
                continue;
            }

            // 2. Atomic claim: transition PENDING -> RUNNING to eliminate concurrency races
            let claimed = crate::db::claim_campaign_execution(conn, exec_id, &now_iso)
                .map_err(|e| format!("DB error claiming execution {}: {}", exec_id, e))?;

            if !claimed {
                // Already claimed or executed by another concurrent worker instance
                continue;
            }

            // 3. Load step definition
            let step = match crate::db::get_campaign_step(conn, execution.step_id)
                .map_err(|e| format!("DB error loading step: {}", e))?
            {
                Some(s) => s,
                None => {
                    crate::db::update_campaign_execution_status(
                        conn,
                        exec_id,
                        "FAILED",
                        Some("Campaign step definition not found"),
                        Some(&now_iso),
                    )
                    .map_err(|e| format!("DB error updating execution to FAILED: {}", e))?;
                    summary.failed_count += 1;
                    continue;
                }
            };

            // 4. Evaluate step condition
            let condition_result =
                StepConditionEvaluator::evaluate(conn, &step, execution.lead_id, Some(exec_id))?;

            match condition_result {
                ConditionResult::Skip { reason } => {
                    crate::db::update_campaign_execution_status(
                        conn,
                        exec_id,
                        "SKIPPED",
                        Some(&reason),
                        Some(&now_iso),
                    )
                    .map_err(|e| format!("DB error updating execution to SKIPPED: {}", e))?;
                    summary.skipped_count += 1;
                    continue;
                }
                ConditionResult::Execute => {
                    // 5. Select account and check anti-ban throttling
                    let account = self
                        .rotator
                        .next_account(&step.channel, &self.throttler)
                        .or_else(|| execution.target_account_id.clone());

                    if let Some(ref acc) = account {
                        if let Err(throttle_err) = self.throttler.can_send(acc) {
                            tracing::warn!(
                                target: "campaign_scheduler",
                                account = %acc,
                                error = %throttle_err,
                                "Account throttled; delaying execution"
                            );
                            // Release claim back to PENDING so it can be retried when rate limit lifts
                            crate::db::update_campaign_execution_status(
                                conn,
                                exec_id,
                                "PENDING",
                                Some("Throttled; waiting for rate limit"),
                                None,
                            )
                            .map_err(|e| {
                                format!("DB error resetting throttled execution: {}", e)
                            })?;
                            summary.throttled_count += 1;
                            continue;
                        }
                    }

                    // 6. Dispatch message
                    let payload = DispatchPayload {
                        campaign_id: execution.campaign_id,
                        step_id: step.id.unwrap_or(0),
                        lead_id: execution.lead_id,
                        channel: step.channel.clone(),
                        target_recipient: execution.target_phone.clone().unwrap_or_default(),
                        account_id: account.clone(),
                        template_content: step.template_content.clone(),
                        ai_prompt_override: step.ai_prompt_override.clone(),
                    };

                    match OmnichannelDispatcher::dispatch(conn, &payload, mode) {
                        Ok(dispatch_res) => {
                            if dispatch_res.success {
                                crate::db::update_campaign_execution_status(
                                    conn,
                                    exec_id,
                                    "SENT",
                                    None,
                                    Some(&now_iso),
                                )
                                .map_err(|e| {
                                    format!("DB error updating execution to SENT: {}", e)
                                })?;

                                if let Some(ref acc) = account {
                                    self.throttler.record_sent(acc);
                                }

                                // Update lead journey state
                                if let Err(e) = crate::db::update_lead_journey_state(
                                    conn,
                                    execution.lead_id,
                                    "OUTREACH",
                                    Some(execution.campaign_id),
                                    None,
                                    Some(&now_iso),
                                    None,
                                    None,
                                    None,
                                ) {
                                    eprintln!("[CampaignScheduler] Warning: failed to update lead journey state: {}", e);
                                }

                                summary.sent_count += 1;

                                // 7. Schedule next sequential step if present
                                let next_step_opt: Option<(i64, i32)> = conn
                                    .query_row(
                                        "SELECT id, delay_minutes FROM campaign_steps
                                         WHERE campaign_id = ?1 AND step_order = ?2
                                         ORDER BY step_order ASC LIMIT 1",
                                        rusqlite::params![step.campaign_id, step.step_order + 1],
                                        |row| Ok((row.get(0)?, row.get(1)?)),
                                    )
                                    .ok();

                                if let Some((next_step_id, delay_minutes)) = next_step_opt {
                                    let next_scheduled_iso: String = conn
                                        .query_row(
                                            "SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now', ?1)",
                                            [format!("+{} minutes", delay_minutes)],
                                            |row| row.get(0),
                                        )
                                        .unwrap_or_else(|_| now_iso.clone());

                                    crate::db::create_campaign_execution(
                                        conn,
                                        execution.campaign_id,
                                        next_step_id,
                                        execution.lead_id,
                                        execution.target_phone.as_deref(),
                                        account.as_deref(),
                                        &next_scheduled_iso,
                                    )
                                    .map_err(|e| {
                                        format!("DB error scheduling next step execution: {}", e)
                                    })?;
                                    summary.next_steps_scheduled += 1;
                                }

                                // Publish event
                                if let Some(ref bus) = self.event_bus {
                                    let _ = bus.publish(AppEvent::OutboundMessageDispatched {
                                        platform: step.channel.clone(),
                                        recipient: execution
                                            .target_phone
                                            .clone()
                                            .unwrap_or_default(),
                                        message_id: dispatch_res.message_id,
                                        status: dispatch_res.delivery_status.clone(),
                                    });
                                }
                            } else {
                                let err_msg = dispatch_res
                                    .error
                                    .unwrap_or_else(|| "Unknown dispatch failure".to_string());
                                crate::db::update_campaign_execution_status(
                                    conn,
                                    exec_id,
                                    "FAILED",
                                    Some(&err_msg),
                                    Some(&now_iso),
                                )
                                .map_err(|e| {
                                    format!("DB error updating execution to FAILED: {}", e)
                                })?;
                                summary.failed_count += 1;
                            }
                        }
                        Err(err) => {
                            // Retry semantics with backoff: up to 3 retries
                            let current_err = execution.error_message.as_deref().unwrap_or("");
                            let retry_count = if current_err.starts_with("[Retry ") {
                                current_err[7..8].parse::<u32>().unwrap_or(0)
                            } else {
                                0
                            };

                            if retry_count < 3 {
                                let next_retry = retry_count + 1;
                                let retry_delay_mins = next_retry * 5;
                                let retry_iso: String = conn
                                    .query_row(
                                        "SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now', ?1)",
                                        [format!("+{} minutes", retry_delay_mins)],
                                        |row| row.get(0),
                                    )
                                    .unwrap_or_else(|_| now_iso.clone());

                                let new_err_msg = format!("[Retry {}/3] {}", next_retry, err);
                                conn.execute(
                                    "UPDATE campaign_executions
                                     SET status = 'PENDING', scheduled_at = ?1, error_message = ?2, executed_at = NULL
                                     WHERE id = ?3",
                                    rusqlite::params![retry_iso, new_err_msg, exec_id],
                                ).map_err(|e| format!("DB error setting execution retry: {}", e))?;
                                summary.failed_count += 1;
                            } else {
                                let terminal_err = format!("[Max Retries Exceeded] {}", err);
                                crate::db::update_campaign_execution_status(
                                    conn,
                                    exec_id,
                                    "FAILED",
                                    Some(&terminal_err),
                                    Some(&now_iso),
                                )
                                .map_err(|e| {
                                    format!("DB error updating execution to FAILED: {}", e)
                                })?;
                                summary.failed_count += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(summary)
    }

    /// Spawns the background scheduler loop in a dedicated tokio async task.
    pub fn start_background_worker(
        self: Arc<Self>,
        db_path: String,
        poll_interval: Duration,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(poll_interval);
            loop {
                interval.tick().await;

                let db_path_clone = db_path.clone();
                let scheduler = self.clone();

                let _ = tokio::task::spawn_blocking(move || {
                    match rusqlite::Connection::open(&db_path_clone) {
                        Ok(conn) => {
                            if let Err(e) = conn.busy_timeout(std::time::Duration::from_secs(5)) {
                                eprintln!("[CampaignScheduler] Failed to set busy timeout: {}", e);
                            }
                            if let Err(e) = conn.execute_batch(
                                "PRAGMA foreign_keys = ON;
                                 PRAGMA journal_mode = WAL;
                                 PRAGMA synchronous = NORMAL;",
                            ) {
                                eprintln!("[CampaignScheduler] Failed to set PRAGMAs: {}", e);
                            }
                            match scheduler.process_due_executions(&conn, DispatchMode::Live, 50) {
                                Ok(summary) => {
                                    if summary.total_due_found > 0 {
                                        println!(
                                            "[CampaignScheduler] Processed cycle: due={}, sent={}, skipped={}, throttled={}, failed={}",
                                            summary.total_due_found,
                                            summary.sent_count,
                                            summary.skipped_count,
                                            summary.throttled_count,
                                            summary.failed_count
                                        );
                                    }
                                }
                                Err(err) => {
                                    eprintln!("[CampaignScheduler] Execution cycle error: {}", err);
                                }
                            }
                        }
                        Err(err) => {
                            eprintln!("[CampaignScheduler] Failed to open SQLite database at {}: {}", db_path_clone, err);
                        }
                    }
                })
                .await;
            }
        })
    }
}
