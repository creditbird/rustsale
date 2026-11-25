//! Conversation actions that need no channel account: timelines, identity links, deletes, quick
//! templates, forwarding status, mentions, pins and delta replay. Sending is wrapped but never run
//! by the tests; channel-bound commands stay with the conversation runtime.

use crate::error::CoreApiError;
use crate::runtime::{
    args, body, checked_id64, checked_optional_id64, id64, limit, on_blocking, state_of,
};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::conversation::commands::{self, ConversationState};
use rustsale_core::conversation::types::{
    Channel, ConversationSendPartContent, CreateConversationQuickTemplateRequest,
    DeleteConversationQuickTemplateRequest, DeleteConversationRequest, LinkIdentityRequest,
    ListConversationMentionCandidatesRequest, ListConversationQuickTemplatesRequest,
    MarkConversationQuickTemplateUsedRequest, PinMessageRequest, RelinkLeadRequest,
    SendConversationBatchRequest, SendConversationMessageRequest, TimelineCursor,
    UnlinkIdentityRequest, UnlinkLeadRequest, UnpinMessageRequest,
    UpdateConversationQuickTemplateRequest,
};

/// A JS number as a delta sequence; negative and non-finite values start at 0.
fn sequence(value: f64) -> u64 {
    if value.is_finite() && value > 0.0 {
        value as u64
    } else {
        0
    }
}

/// A JS number as a row count, absent when it is not a positive number.
fn count(value: Option<f64>) -> Option<usize> {
    value
        .filter(|n| n.is_finite() && *n > 0.0)
        .map(|n| n.min(f64::from(u32::MAX)) as usize)
}

/// An optional timeline cursor: absent or `null` is the first page.
fn cursor(before: Option<JsonValue>) -> Result<Option<TimelineCursor>, CoreApiError> {
    match before {
        None | Some(JsonValue::Null) => Ok(None),
        Some(value) => {
            let mut cursor: TimelineCursor = body(value)?;
            cursor.message_id = checked_id64(cursor.message_id)?;
            Ok(Some(cursor))
        }
    }
}

/// Checks the one target of a delete request serde accepted.
fn check_delete(request: &mut DeleteConversationRequest) -> Result<(), CoreApiError> {
    request.conversation_id = checked_optional_id64(request.conversation_id)?;
    request.identity_id = checked_optional_id64(request.identity_id)?;
    request.lead_id = checked_optional_id64(request.lead_id)?;
    Ok(())
}

/// The conversation commands that run without a connected account.
#[module]
pub struct ConversationActions;

#[module]
impl ConversationActions {
    /// The timeline of lead `leadId` over `channels` (`["zalo", "whatsapp", "messenger", "email"]`;
    /// absent means none, hence an empty page). `before` is the previous `nextCursor`.
    pub async fn contact_timeline(
        lead_id: JsonValue,
        channels: Option<JsonValue>,
        before: Option<JsonValue>,
        limit_: Option<f64>,
    ) -> Result<JsonValue, JsError> {
        let channels: Vec<Channel> = args(channels)?;
        let before = cursor(before)?;
        let limit = limit(limit_);
        let lead = id64(lead_id)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_contact_timeline(
                state, lead, channels, before, limit,
            )?)
        })
        .await
    }

    /// The timeline of linked identity `identityId`; `before` is the previous `nextCursor`.
    pub async fn identity_timeline(
        identity_id: JsonValue,
        before: Option<JsonValue>,
        limit_: Option<f64>,
    ) -> Result<JsonValue, JsError> {
        let before = cursor(before)?;
        let limit = limit(limit_);
        let identity = id64(identity_id)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_identity_timeline(
                state, identity, before, limit,
            )?)
        })
        .await
    }

    /// Links identity `request` (`{ identityId, leadId }`) to its lead.
    pub async fn link_identity(request: JsonValue) -> Result<JsonValue, JsError> {
        let mut request: LinkIdentityRequest = body(request)?;
        request.identity_id = checked_id64(request.identity_id)?;
        request.lead_id = checked_id64(request.lead_id)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::link_conversation_identity(state, request)?)
        })
        .await
    }

    /// Unlinks identity `request` (`{ identityId }`); its messages stay.
    pub async fn unlink_identity(request: JsonValue) -> Result<JsonValue, JsError> {
        let mut request: UnlinkIdentityRequest = body(request)?;
        request.identity_id = checked_id64(request.identity_id)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::unlink_conversation_identity(state, request)?)
        })
        .await
    }

    /// Unlinks lead `request` (`{ leadId }`) from its conversations; the conversations stay.
    pub async fn unlink_lead(request: JsonValue) -> Result<JsonValue, JsError> {
        let mut request: UnlinkLeadRequest = body(request)?;
        request.lead_id = checked_id64(request.lead_id)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::unlink_conversation_lead(state, request)?)
        })
        .await
    }

    /// Moves the link of `request` (`{ sourceLeadId, targetLeadId }`) to another lead.
    pub async fn relink_lead(request: JsonValue) -> Result<JsonValue, JsError> {
        let mut request: RelinkLeadRequest = body(request)?;
        request.source_lead_id = checked_id64(request.source_lead_id)?;
        request.target_lead_id = checked_id64(request.target_lead_id)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::relink_conversation_lead(state, request)?)
        })
        .await
    }

    /// Queues one message from `request` in the outbox; the channel supervisor delivers it. Fails
    /// with `not_found` for a missing conversation or account, `invalid_request` for an account
    /// that is not the conversation's and `unavailable` when that account is not connected; nothing
    /// is queued then.
    pub async fn send_message(request: JsonValue) -> Result<JsonValue, JsError> {
        let mut request: SendConversationMessageRequest = body(request)?;
        request.conversation_id = checked_id64(request.conversation_id)?;
        request.channel_account_id = checked_id64(request.channel_account_id)?;
        request.reply_to_message_id = checked_optional_id64(request.reply_to_message_id)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            // Before anything is queued: a missing target is `not_found`, an account that is not
            // connected is `unavailable`, and a wrong account is never swapped for another.
            commands::check_send_target(&state, &request)?;
            Ok::<_, CoreApiError>(commands::send_conversation_message(state, request)?)
        })
        .await
    }

    /// Queues a message of several parts from `request` in the outbox; only the `HUMAN_AGENT` tag
    /// is accepted.
    pub async fn send_batch(request: JsonValue) -> Result<JsonValue, JsError> {
        let mut request: SendConversationBatchRequest = body(request)?;
        request.conversation_id = checked_id64(request.conversation_id)?;
        request.reply_to_message_id = checked_optional_id64(request.reply_to_message_id)?;
        for part in &mut request.parts {
            match &mut part.content {
                ConversationSendPartContent::Text { mentions, .. } => {
                    for mention in mentions {
                        mention.target_identity_id =
                            checked_optional_id64(mention.target_identity_id)?;
                    }
                }
                ConversationSendPartContent::Forward { source_message_id } => {
                    *source_message_id = checked_id64(*source_message_id)?;
                }
                _ => {}
            }
        }
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::send_conversation_batch(state, request)?)
        })
        .await
    }

    /// Deletes one conversation from `request` (`{ conversationId? | identityId? | leadId? }`): exactly
    /// one of the three must be given.
    pub async fn delete_conversation(request: JsonValue) -> Result<JsonValue, JsError> {
        let mut request: DeleteConversationRequest = body(request)?;
        check_delete(&mut request)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_conversation(state, request)?)
        })
        .await
    }

    /// Deletes every conversation in `requests`, each shaped as `deleteConversation` takes it.
    pub async fn delete_conversations(requests: JsonValue) -> Result<JsonValue, JsError> {
        let mut requests: Vec<DeleteConversationRequest> = body(requests)?;
        for request in &mut requests {
            check_delete(request)?;
        }
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_conversations_batch(state, requests)?)
        })
        .await
    }

    /// The quick reply templates matching `request` (`{ category?, searchQuery?, includeInactive? }`).
    pub async fn quick_templates(request: Option<JsonValue>) -> Result<JsonValue, JsError> {
        let request = match request {
            None | Some(JsonValue::Null) => ListConversationQuickTemplatesRequest {
                category: None,
                search_query: None,
                include_inactive: false,
            },
            Some(value) => body(value)?,
        };
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::list_conversation_quick_templates(state, request)?)
        })
        .await
    }

    /// Creates a quick reply template from `request` (`{ shortcut, title, bodyText, category? }`).
    pub async fn create_quick_template(request: JsonValue) -> Result<JsonValue, JsError> {
        let request: CreateConversationQuickTemplateRequest = body(request)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::create_conversation_quick_template(
                state, request,
            )?)
        })
        .await
    }

    /// Replaces quick reply template `request` (`{ id, shortcut, title, bodyText, category?,
    /// isActive? }`).
    pub async fn update_quick_template(request: JsonValue) -> Result<JsonValue, JsError> {
        let mut request: UpdateConversationQuickTemplateRequest = body(request)?;
        request.id = checked_id64(request.id)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::update_conversation_quick_template(
                state, request,
            )?)
        })
        .await
    }

    /// Deletes quick reply template `request` (`{ id }`).
    pub async fn delete_quick_template(request: JsonValue) -> Result<JsonValue, JsError> {
        let mut request: DeleteConversationQuickTemplateRequest = body(request)?;
        request.id = checked_id64(request.id)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_conversation_quick_template(
                state, request,
            )?)
        })
        .await
    }

    /// Counts one more use of quick reply template `request` (`{ id }`).
    pub async fn mark_quick_template_used(request: JsonValue) -> Result<JsonValue, JsError> {
        let mut request: MarkConversationQuickTemplateUsedRequest = body(request)?;
        request.id = checked_id64(request.id)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::mark_conversation_quick_template_used(
                state, request,
            )?)
        })
        .await
    }

    /// The state of forward operation `clientOperationId`; a missing one rejects.
    pub async fn forward_operation(client_operation_id: String) -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_conversation_forward_operation(
                state,
                client_operation_id,
            )?)
        })
        .await
    }

    /// The members `request` (`{ conversationId, sourceMessageId?, query?, limit? }`) can mention.
    pub async fn mention_candidates(request: JsonValue) -> Result<JsonValue, JsError> {
        let mut request: ListConversationMentionCandidatesRequest = body(request)?;
        request.conversation_id = checked_id64(request.conversation_id)?;
        request.source_message_id = checked_optional_id64(request.source_message_id)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::list_conversation_mention_candidates(
                state, request,
            )?)
        })
        .await
    }

    /// Pins message `request` (`{ conversationId, messageId, pinnedBy? }`).
    pub async fn pin_message(request: JsonValue) -> Result<JsonValue, JsError> {
        let mut request: PinMessageRequest = body(request)?;
        request.conversation_id = checked_id64(request.conversation_id)?;
        request.message_id = checked_id64(request.message_id)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::pin_conversation_message(state, request)?)
        })
        .await
    }

    /// Unpins message `request` (`{ conversationId, messageId }`).
    pub async fn unpin_message(request: JsonValue) -> Result<JsonValue, JsError> {
        let mut request: UnpinMessageRequest = body(request)?;
        request.conversation_id = checked_id64(request.conversation_id)?;
        request.message_id = checked_id64(request.message_id)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::unpin_conversation_message(state, request)?)
        })
        .await
    }

    /// The pinned messages of conversation `conversationId`, newest first.
    pub async fn pinned_messages(conversation_id: JsonValue) -> Result<JsonValue, JsError> {
        let conversation = id64(conversation_id)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::list_conversation_pinned_messages(
                state,
                conversation,
            )?)
        })
        .await
    }

    /// The message deltas after `afterSequence`, oldest first; `limit` absent means 200, the most 500.
    pub async fn resume_deltas(
        after_sequence: f64,
        limit_: Option<f64>,
    ) -> Result<JsonValue, JsError> {
        let after_sequence = sequence(after_sequence);
        let limit = count(limit_);
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::resume_conversation_deltas(
                state,
                after_sequence,
                limit,
            )?)
        })
        .await
    }

    /// The highest and lowest sequence of the conversation delta log.
    pub async fn high_water_mark() -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_conversation_high_water_mark(state)?)
        })
        .await
    }
}
