use crate::ai::evidence::{EvidenceExtractor, LeadEvidenceContext};
use crate::ai::gateway::LlmGateway;
use crate::ai::generator::{SmartReplyGenerator, SmartReplyOutput};
use crate::ai::intent::{CustomerIntent, IntentClassifier, IntentResult};
use crate::ai::policy::PolicyGate;
use crate::commands::DbState;
use crate::models::AiActionProposal;
use rusqlite::Connection;

#[tauri::command]
pub async fn classify_customer_intent(text: String) -> Result<IntentResult, String> {
    let gateway = LlmGateway::from_env();
    Ok(IntentClassifier::classify(&text, Some(&gateway)).await)
}

#[tauri::command]
pub async fn generate_smart_reply_suggestions(
    lead_id: i32,
    conversation_id: Option<i64>,
    custom_prompt: Option<String>,
    state: tauri::State<'_, DbState>,
) -> Result<SmartReplyOutput, String> {
    let gateway = LlmGateway::from_env();
    let policy_gate = PolicyGate::with_default_config();

    // 1. Extract context with scoped lock so MutexGuard is dropped before .await
    let context = {
        let conn = state
            .conn
            .lock()
            .map_err(|e| format!("Database lock error: {}", e))?;
        EvidenceExtractor::extract_lead_context(&conn, lead_id, conversation_id)?
    };

    // 2. Classify intent
    let last_inbound = context
        .recent_messages
        .iter()
        .rev()
        .find(|m| m.direction == "inbound")
        .map(|m| m.text_content.as_str());

    let text_to_classify = last_inbound.or(custom_prompt.as_deref()).unwrap_or("");

    let intent = if !text_to_classify.trim().is_empty() {
        IntentClassifier::classify(text_to_classify, Some(&gateway)).await
    } else {
        IntentResult {
            intent: CustomerIntent::GeneralInquiry,
            confidence: 0.80,
            detected_language: "vi".to_string(),
            matched_keywords: vec![],
            reasoning: "Defaulted to general inquiry (no recent inbound message)".to_string(),
            is_heuristic_match: true,
        }
    };

    // 3. Generate suggestions (async)
    let mut output = SmartReplyGenerator::generate(
        &context,
        &intent,
        custom_prompt.as_deref(),
        &gateway,
        &policy_gate,
    )
    .await?;

    output.conversation_id = conversation_id;

    // 4. Record evidence and update journey state in scoped lock
    {
        let conn = state
            .conn
            .lock()
            .map_err(|e| format!("Database lock error: {}", e))?;

        let evidence_data = serde_json::json!({
            "lead_name": context.lead_name,
            "lead_company": context.lead_company,
            "deal_stage": context.deal_stage,
            "deal_value": context.deal_value,
            "intent": intent.intent.as_str(),
            "confidence": intent.confidence,
            "variants_count": output.variants.len()
        });

        let top_decision = output
            .variants
            .first()
            .map(|v| v.policy_decision.as_str())
            .unwrap_or("REQUIRE_REVIEW");

        let _ = crate::db::insert_ai_evidence_log(
            &conn,
            Some(lead_id),
            conversation_id,
            "GENERATE_SMART_REPLY",
            "CRM_LEDGER_AND_CONVERSATION",
            &evidence_data.to_string(),
            intent.confidence,
            top_decision,
        );

        let current_stage = context.journey_stage.as_deref().unwrap_or("DISCOVERY");
        let _ = crate::db::update_lead_journey_state(
            &conn,
            lead_id,
            current_stage,
            None,
            None,
            None,
            Some(intent.intent.as_str()),
            Some(intent.confidence),
            None,
        );
    }

    Ok(output)
}

#[tauri::command]
pub fn get_pending_ai_proposals(
    state: tauri::State<'_, DbState>,
) -> Result<Vec<AiActionProposal>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;

    get_pending_ai_proposals_internal(&conn)
}

#[tauri::command]
pub fn review_ai_proposal(
    proposal_id: i64,
    status: String,
    reviewer: String,
    state: tauri::State<'_, DbState>,
) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;

    review_ai_proposal_internal(&conn, proposal_id, &status, &reviewer)
}

#[tauri::command]
pub fn get_lead_evidence_context(
    lead_id: i32,
    state: tauri::State<'_, DbState>,
) -> Result<LeadEvidenceContext, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;

    get_lead_evidence_context_internal(&conn, lead_id)
}

// ============================================================================
// Internal Handlers (Testable without Tauri Runtime)
// ============================================================================

pub async fn generate_smart_reply_suggestions_internal(
    conn: &Connection,
    lead_id: i32,
    conversation_id: Option<i64>,
    custom_prompt: Option<String>,
    gateway: &LlmGateway,
    policy_gate: &PolicyGate,
) -> Result<SmartReplyOutput, String> {
    // 1. Extract factual CRM context
    let context = EvidenceExtractor::extract_lead_context(conn, lead_id, conversation_id)?;

    // 2. Determine intent from last inbound message or custom prompt
    let last_inbound = context
        .recent_messages
        .iter()
        .rev()
        .find(|m| m.direction == "inbound")
        .map(|m| m.text_content.as_str());

    let text_to_classify = last_inbound.or(custom_prompt.as_deref()).unwrap_or("");

    let intent = if !text_to_classify.trim().is_empty() {
        IntentClassifier::classify(text_to_classify, Some(gateway)).await
    } else {
        IntentResult {
            intent: CustomerIntent::GeneralInquiry,
            confidence: 0.80,
            detected_language: "vi".to_string(),
            matched_keywords: vec![],
            reasoning: "Defaulted to general inquiry (no recent inbound message)".to_string(),
            is_heuristic_match: true,
        }
    };

    // 3. Generate suggestions with safety verification
    let mut output = SmartReplyGenerator::generate(
        &context,
        &intent,
        custom_prompt.as_deref(),
        gateway,
        policy_gate,
    )
    .await?;

    output.conversation_id = conversation_id;

    // 4. Log evidence ledger entry in DB
    let evidence_data = serde_json::json!({
        "lead_name": context.lead_name,
        "lead_company": context.lead_company,
        "deal_stage": context.deal_stage,
        "deal_value": context.deal_value,
        "intent": intent.intent.as_str(),
        "confidence": intent.confidence,
        "variants_count": output.variants.len()
    });

    let top_decision = output
        .variants
        .first()
        .map(|v| v.policy_decision.as_str())
        .unwrap_or("REQUIRE_REVIEW");

    let _ = crate::db::insert_ai_evidence_log(
        conn,
        Some(lead_id),
        conversation_id,
        "GENERATE_SMART_REPLY",
        "CRM_LEDGER_AND_CONVERSATION",
        &evidence_data.to_string(),
        intent.confidence,
        top_decision,
    );

    // 5. Update lead journey state AI intent tag
    let current_stage = context.journey_stage.as_deref().unwrap_or("DISCOVERY");
    let _ = crate::db::update_lead_journey_state(
        conn,
        lead_id,
        current_stage,
        None,
        None,
        None,
        Some(intent.intent.as_str()),
        Some(intent.confidence),
        None,
    );

    Ok(output)
}

pub fn get_pending_ai_proposals_internal(
    conn: &Connection,
) -> Result<Vec<AiActionProposal>, String> {
    crate::db::get_ai_action_proposals(conn, Some("PENDING"))
        .map_err(|e| format!("Failed to get pending AI proposals: {}", e))
}

pub fn review_ai_proposal_internal(
    conn: &Connection,
    proposal_id: i64,
    status: &str,
    reviewer: &str,
) -> Result<(), String> {
    let valid_statuses = ["APPROVED", "REJECTED", "APPLIED", "PENDING"];
    let status_upper = status.trim().to_uppercase();
    if !valid_statuses.contains(&status_upper.as_str()) {
        return Err(format!(
            "Invalid status '{}'. Must be one of: {:?}",
            status, valid_statuses
        ));
    }

    crate::db::update_ai_action_proposal_status(
        conn,
        proposal_id,
        &status_upper,
        Some(reviewer.trim()),
    )
    .map_err(|e| format!("Failed to update proposal status: {}", e))
}

pub fn get_lead_evidence_context_internal(
    conn: &Connection,
    lead_id: i32,
) -> Result<LeadEvidenceContext, String> {
    EvidenceExtractor::extract_lead_context(conn, lead_id, None)
}
