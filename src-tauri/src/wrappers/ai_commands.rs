#![allow(
    unused_imports,
    unused_variables,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
use crate::CoreHandle;
#[allow(unused_imports)]
use rustsale_core::ai::commands::*;
use rustsale_core::ai::evidence::{EvidenceExtractor, LeadEvidenceContext};
use rustsale_core::ai::gateway::LlmGateway;
use rustsale_core::ai::generator::{SmartReplyGenerator, SmartReplyOutput};
use rustsale_core::ai::intent::{CustomerIntent, IntentClassifier, IntentResult};
use rustsale_core::ai::policy::PolicyGate;
use rustsale_core::commands::DbState;
use rustsale_core::host::{AppHandle, State};
use rustsale_core::models::AiActionProposal;

#[tauri::command]
pub async fn classify_customer_intent(
    core: tauri::State<'_, CoreHandle>,
    text: String,
) -> Result<IntentResult, String> {
    let app = core.handle();
    rustsale_core::ai::commands::classify_customer_intent(text).await
}

#[tauri::command]
pub async fn generate_smart_reply_suggestions(
    core: tauri::State<'_, CoreHandle>,
    lead_id: i32,
    conversation_id: Option<i64>,
    custom_prompt: Option<String>,
) -> Result<SmartReplyOutput, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::ai::commands::generate_smart_reply_suggestions(
        lead_id,
        conversation_id,
        custom_prompt,
        st_state,
    )
    .await
}

#[tauri::command]
pub fn get_pending_ai_proposals(
    core: tauri::State<'_, CoreHandle>,
) -> Result<Vec<AiActionProposal>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::ai::commands::get_pending_ai_proposals(st_state)
}

#[tauri::command]
pub fn review_ai_proposal(
    core: tauri::State<'_, CoreHandle>,
    proposal_id: i64,
    status: String,
    reviewer: String,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::ai::commands::review_ai_proposal(proposal_id, status, reviewer, st_state)
}

#[tauri::command]
pub fn get_lead_evidence_context(
    core: tauri::State<'_, CoreHandle>,
    lead_id: i32,
) -> Result<LeadEvidenceContext, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::ai::commands::get_lead_evidence_context(lead_id, st_state)
}
