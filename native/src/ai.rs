//! The AI copilot: intent, reply suggestions, proposals and the lead evidence context.

use crate::error::CoreApiError;
use crate::runtime::{id32, id64, on_blocking, on_core, optional_id64, state_of};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::ai::commands as ai;
use rustsale_core::commands::DbState;

/// A JS number as a lead id, refusing anything that is not a whole, non-negative `i32`.
fn lead_id(value: f64) -> Result<i32, CoreApiError> {
    id32(value)
}

/// The local AI copilot. The two generation calls use the provider the core was configured with
/// (`OPENAI_API_KEY`, Gemini, Claude, Ollama or the heuristic mock); the rest read the CRM ledger.
#[module]
pub struct Ai;

#[module]
impl Ai {
    /// Classifies the intent of `text` and resolves `{ intent, confidence, detected_language,
    /// matched_keywords, reasoning, is_heuristic_match }`. Falls back to the heuristic classifier
    /// when no provider is configured.
    pub async fn classify_intent(text: String) -> Result<JsonValue, JsError> {
        on_core(move |_h| async move {
            Ok::<_, CoreApiError>(ai::classify_customer_intent(text).await?)
        })
        .await
    }

    /// Generates reply suggestions for lead `leadId` from the CRM evidence and the policy gate.
    /// `conversationId` and `customPrompt` may be absent; the answer is `{ conversation_id,
    /// variants, ... }` as the core returns it.
    pub async fn smart_reply_suggestions(
        lead_id_: f64,
        conversation_id: Option<f64>,
        custom_prompt: Option<String>,
    ) -> Result<JsonValue, JsError> {
        let lead = lead_id(lead_id_)?;
        let conversation = optional_id64(conversation_id)?;
        on_core(move |h| async move {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(
                ai::generate_smart_reply_suggestions(lead, conversation, custom_prompt, state)
                    .await?,
            )
        })
        .await
    }

    /// AI action proposals waiting for review.
    pub async fn pending_proposals() -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(ai::get_pending_ai_proposals(state)?)
        })
        .await
    }

    /// Reviews proposal `proposalId` as `APPROVED`, `REJECTED`, `APPLIED` or `PENDING`, recording
    /// `reviewer`.
    pub async fn review_proposal(
        proposal_id: f64,
        status: String,
        reviewer: String,
    ) -> Result<JsonValue, JsError> {
        let proposal = id64(proposal_id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(ai::review_ai_proposal(proposal, status, reviewer, state)?)
        })
        .await
    }

    /// The CRM evidence of lead `leadId` that the copilot reads: the lead, its deal, recent
    /// messages and the journey stage.
    pub async fn lead_evidence_context(lead_id_: f64) -> Result<JsonValue, JsError> {
        let lead = lead_id(lead_id_)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(ai::get_lead_evidence_context(lead, state)?)
        })
        .await
    }
}
