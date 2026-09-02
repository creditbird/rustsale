use crate::models::CampaignStep;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionResult {
    Execute,
    Skip { reason: String },
}

impl ConditionResult {
    pub fn should_execute(&self) -> bool {
        matches!(self, ConditionResult::Execute)
    }
}

pub struct StepConditionEvaluator;

impl StepConditionEvaluator {
    /// Evaluates whether a campaign step condition is satisfied for a given lead.
    /// Supports condition types:
    /// - `ALWAYS`: Always executes.
    /// - `IF_NOT_REPLIED`: Skips if customer replied since previous step/enrollment.
    /// - `IF_DEAL_STAGE_IS[:<STAGE>]`: Checks if current deal stage matches expected stage.
    /// - `IF_INTENT_IS[:<INTENT>]`: Checks if lead AI intent tag matches expected intent.
    pub fn evaluate(
        conn: &Connection,
        step: &CampaignStep,
        lead_id: i32,
        _execution_id: Option<i64>,
    ) -> Result<ConditionResult, String> {
        let condition = step.condition_type.trim().to_uppercase();

        if condition.is_empty() || condition == "ALWAYS" {
            return Ok(ConditionResult::Execute);
        }

        if condition == "IF_NOT_REPLIED" {
            return Self::evaluate_if_not_replied(conn, step, lead_id);
        }

        if condition.starts_with("IF_DEAL_STAGE_IS") {
            let parts: Vec<&str> = condition.split(':').collect();
            let expected_stage = if parts.len() > 1 && !parts[1].trim().is_empty() {
                parts[1].trim()
            } else {
                "QUALIFIED"
            };
            return Self::evaluate_if_deal_stage_is(conn, lead_id, expected_stage);
        }

        if condition.starts_with("IF_INTENT_IS") {
            let parts: Vec<&str> = condition.split(':').collect();
            let expected_intent = if parts.len() > 1 && !parts[1].trim().is_empty() {
                parts[1].trim()
            } else {
                "GENERALINQUIRY"
            };
            return Self::evaluate_if_intent_is(conn, lead_id, expected_intent);
        }

        // Unknown condition defaults to Execute
        Ok(ConditionResult::Execute)
    }

    /// Checks if customer has replied to any message since outreach was initiated or previous step executed.
    fn evaluate_if_not_replied(
        conn: &Connection,
        step: &CampaignStep,
        lead_id: i32,
    ) -> Result<ConditionResult, String> {
        // 1. Check lead_journey_states
        if let Ok(Some(journey)) = crate::db::get_lead_journey_state(conn, lead_id) {
            if let Some(inbound_at) = journey.last_inbound_at {
                if let Some(outbound_at) = journey.last_outbound_at {
                    if inbound_at > outbound_at {
                        return Ok(ConditionResult::Skip {
                            reason: format!(
                                "Customer replied at {} after last outreach at {}",
                                inbound_at, outbound_at
                            ),
                        });
                    }
                } else {
                    // There is an inbound message and no recorded outbound -> customer replied
                    return Ok(ConditionResult::Skip {
                        reason: format!("Customer has inbound message recorded at {}", inbound_at),
                    });
                }
            }
        }

        // 2. Check previous step execution timestamp for this campaign & lead
        let prev_execution_time: Option<String> = conn
            .query_row(
                "SELECT executed_at FROM campaign_executions
                 WHERE campaign_id = ?1 AND lead_id = ?2 AND step_id != ?3 AND status = 'SENT'
                 ORDER BY executed_at DESC LIMIT 1",
                rusqlite::params![step.campaign_id, lead_id, step.id.unwrap_or(0)],
                |row| row.get(0),
            )
            .ok()
            .flatten();

        if let Some(prev_time) = prev_execution_time {
            // Check if there are any inbound conversation messages after previous step execution
            let has_inbound_after: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM conversation_messages cm
                     JOIN conversations c ON cm.conversation_id = c.id
                     WHERE c.lead_id = ?1 AND cm.direction = 'inbound' AND cm.created_at > ?2",
                    rusqlite::params![lead_id, prev_time],
                    |row| row.get(0),
                )
                .unwrap_or(false);

            if has_inbound_after {
                return Ok(ConditionResult::Skip {
                    reason: format!(
                        "Customer sent inbound message after previous step execution at {}",
                        prev_time
                    ),
                });
            }
        }

        Ok(ConditionResult::Execute)
    }

    /// Checks if current deal stage matches expected stage.
    fn evaluate_if_deal_stage_is(
        conn: &Connection,
        lead_id: i32,
        expected_stage: &str,
    ) -> Result<ConditionResult, String> {
        let expected_normalized = expected_stage.trim().to_uppercase();

        // 1. Check deals table for this lead
        let deal_stage: Option<String> = conn
            .query_row(
                "SELECT stage FROM deals WHERE lead_id = ?1 ORDER BY id DESC LIMIT 1",
                [lead_id],
                |row| row.get(0),
            )
            .ok();

        // 2. Check lead_journey_states as fallback
        let journey_stage: Option<String> = conn
            .query_row(
                "SELECT current_stage FROM lead_journey_states WHERE lead_id = ?1",
                [lead_id],
                |row| row.get(0),
            )
            .ok();

        // 3. Check leads.status as additional fallback
        let lead_status: Option<String> = conn
            .query_row(
                "SELECT status FROM leads WHERE id = ?1",
                [lead_id],
                |row| row.get(0),
            )
            .ok();

        let current_stage = deal_stage
            .or(journey_stage)
            .or(lead_status)
            .unwrap_or_else(|| "UNKNOWN".to_string());

        let current_normalized = current_stage.trim().to_uppercase();

        if current_normalized == expected_normalized {
            Ok(ConditionResult::Execute)
        } else {
            Ok(ConditionResult::Skip {
                reason: format!(
                    "Current stage '{}' does not match expected stage '{}'",
                    current_stage, expected_stage
                ),
            })
        }
    }

    /// Checks if lead AI intent tag matches expected intent.
    fn evaluate_if_intent_is(
        conn: &Connection,
        lead_id: i32,
        expected_intent: &str,
    ) -> Result<ConditionResult, String> {
        let expected_normalized = expected_intent
            .trim()
            .to_uppercase()
            .replace('_', "")
            .replace(' ', "");

        let intent_tag: Option<String> = conn
            .query_row(
                "SELECT ai_intent_tag FROM lead_journey_states WHERE lead_id = ?1",
                [lead_id],
                |row| row.get(0),
            )
            .ok()
            .flatten();

        let current_intent = intent_tag.unwrap_or_else(|| "UNKNOWN".to_string());
        let current_normalized = current_intent
            .trim()
            .to_uppercase()
            .replace('_', "")
            .replace(' ', "");

        if current_normalized == expected_normalized {
            Ok(ConditionResult::Execute)
        } else {
            Ok(ConditionResult::Skip {
                reason: format!(
                    "Current intent '{}' does not match expected intent '{}'",
                    current_intent, expected_intent
                ),
            })
        }
    }
}
