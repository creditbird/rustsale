//! Conversation actions that need no channel account: timelines, identity links, deletes, quick
//! templates, forwarding status, mentions, pins and delta replay. Sending is wrapped but never run
//! by the tests; channel-bound commands stay with the conversation runtime.

use crate::error::CoreApiError;
use crate::runtime::{args, body, limit, on_blocking, state_of};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::conversation::commands::{self, ConversationState};
use rustsale_core::conversation::types::{
    Channel, CreateConversationQuickTemplateRequest, DeleteConversationQuickTemplateRequest,
    DeleteConversationRequest, LinkIdentityRequest, ListConversationMentionCandidatesRequest,
    ListConversationQuickTemplatesRequest, MarkConversationQuickTemplateUsedRequest,
    PinMessageRequest, RelinkLeadRequest, SendConversationBatchRequest,
    SendConversationMessageRequest, TimelineCursor, UnlinkIdentityRequest, UnlinkLeadRequest,
    UnpinMessageRequest, UpdateConversationQuickTemplateRequest,
};

/// A JS number as an id; anything not finite is 0, which the core then reports as missing.
fn id(value: f64) -> i64 {
    if value.is_finite() { value as i64 } else { 0 }
}

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
        Some(value) => body(value).map(Some),
    }
}

/// The conversation commands that run without a connected account.
#[module]
pub struct ConversationActions;

#[module]
impl ConversationActions {
    /// The timeline of lead `leadId` over `channels` (`["zalo", "whatsapp", "messenger", "email"]`;
    /// absent means none, hence an empty page). `before` is the previous `nextCursor`.
    pub async fn contact_timeline(
        lead_id: f64,
        channels: Option<JsonValue>,
        before: Option<JsonValue>,
        limit_: Option<f64>,
    ) -> Result<JsonValue, CoreApiError> {
        let channels: Vec<Channel> = args(channels)?;
        let before = cursor(before)?;
        let limit = limit(limit_);
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_contact_timeline(
                state,
                id(lead_id),
                channels,
                before,
                limit,
            )?)
        })
        .await
    }

    /// The timeline of linked identity `identityId`; `before` is the previous `nextCursor`.
    pub async fn identity_timeline(
        identity_id: f64,
        before: Option<JsonValue>,
        limit_: Option<f64>,
    ) -> Result<JsonValue, CoreApiError> {
        let before = cursor(before)?;
        let limit = limit(limit_);
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_identity_timeline(
                state,
                id(identity_id),
                before,
                limit,
            )?)
        })
        .await
    }

    /// Links identity `request` (`{ identityId, leadId }`) to its lead.
    pub async fn link_identity(request: JsonValue) -> Result<JsonValue, CoreApiError> {
        let request: LinkIdentityRequest = body(request)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::link_conversation_identity(state, request)?)
        })
        .await
    }

    /// Unlinks identity `request` (`{ identityId }`); its messages stay.
    pub async fn unlink_identity(request: JsonValue) -> Result<JsonValue, CoreApiError> {
        let request: UnlinkIdentityRequest = body(request)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::unlink_conversation_identity(state, request)?)
        })
        .await
    }

    /// Unlinks lead `request` (`{ leadId }`) from its conversations; the conversations stay.
    pub async fn unlink_lead(request: JsonValue) -> Result<JsonValue, CoreApiError> {
        let request: UnlinkLeadRequest = body(request)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::unlink_conversation_lead(state, request)?)
        })
        .await
    }

    /// Moves the link of `request` (`{ sourceLeadId, targetLeadId }`) to another lead.
    pub async fn relink_lead(request: JsonValue) -> Result<JsonValue, CoreApiError> {
        let request: RelinkLeadRequest = body(request)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::relink_conversation_lead(state, request)?)
        })
        .await
    }

    /// Queues one message from `request` in the outbox; the channel supervisor delivers it.
    pub async fn send_message(request: JsonValue) -> Result<JsonValue, CoreApiError> {
        let request: SendConversationMessageRequest = body(request)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::send_conversation_message(state, request)?)
        })
        .await
    }

    /// Queues a message of several parts from `request` in the outbox; only the `HUMAN_AGENT` tag
    /// is accepted.
    pub async fn send_batch(request: JsonValue) -> Result<JsonValue, CoreApiError> {
        let request: SendConversationBatchRequest = body(request)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::send_conversation_batch(state, request)?)
        })
        .await
    }

    /// Deletes one conversation from `request` (`{ conversationId? | identityId? | leadId? }`): exactly
    /// one of the three must be given.
    pub async fn delete_conversation(request: JsonValue) -> Result<JsonValue, CoreApiError> {
        let request: DeleteConversationRequest = body(request)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_conversation(state, request)?)
        })
        .await
    }

    /// Deletes every conversation in `requests`, each shaped as `deleteConversation` takes it.
    pub async fn delete_conversations(requests: JsonValue) -> Result<JsonValue, CoreApiError> {
        let requests: Vec<DeleteConversationRequest> = body(requests)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_conversations_batch(state, requests)?)
        })
        .await
    }

    /// The quick reply templates matching `request` (`{ category?, searchQuery?, includeInactive? }`).
    pub async fn quick_templates(request: Option<JsonValue>) -> Result<JsonValue, CoreApiError> {
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
    pub async fn create_quick_template(request: JsonValue) -> Result<JsonValue, CoreApiError> {
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
    pub async fn update_quick_template(request: JsonValue) -> Result<JsonValue, CoreApiError> {
        let request: UpdateConversationQuickTemplateRequest = body(request)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::update_conversation_quick_template(
                state, request,
            )?)
        })
        .await
    }

    /// Deletes quick reply template `request` (`{ id }`).
    pub async fn delete_quick_template(request: JsonValue) -> Result<JsonValue, CoreApiError> {
        let request: DeleteConversationQuickTemplateRequest = body(request)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_conversation_quick_template(
                state, request,
            )?)
        })
        .await
    }

    /// Counts one more use of quick reply template `request` (`{ id }`).
    pub async fn mark_quick_template_used(request: JsonValue) -> Result<JsonValue, CoreApiError> {
        let request: MarkConversationQuickTemplateUsedRequest = body(request)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::mark_conversation_quick_template_used(
                state, request,
            )?)
        })
        .await
    }

    /// The state of forward operation `clientOperationId`; a missing one rejects.
    pub async fn forward_operation(client_operation_id: String) -> Result<JsonValue, CoreApiError> {
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
    pub async fn mention_candidates(request: JsonValue) -> Result<JsonValue, CoreApiError> {
        let request: ListConversationMentionCandidatesRequest = body(request)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::list_conversation_mention_candidates(
                state, request,
            )?)
        })
        .await
    }

    /// Pins message `request` (`{ conversationId, messageId, pinnedBy? }`).
    pub async fn pin_message(request: JsonValue) -> Result<JsonValue, CoreApiError> {
        let request: PinMessageRequest = body(request)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::pin_conversation_message(state, request)?)
        })
        .await
    }

    /// Unpins message `request` (`{ conversationId, messageId }`).
    pub async fn unpin_message(request: JsonValue) -> Result<JsonValue, CoreApiError> {
        let request: UnpinMessageRequest = body(request)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::unpin_conversation_message(state, request)?)
        })
        .await
    }

    /// The pinned messages of conversation `conversationId`, newest first.
    pub async fn pinned_messages(conversation_id: f64) -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::list_conversation_pinned_messages(
                state,
                id(conversation_id),
            )?)
        })
        .await
    }

    /// The message deltas after `afterSequence`, oldest first; `limit` absent means 200, the most 500.
    pub async fn resume_deltas(
        after_sequence: f64,
        limit_: Option<f64>,
    ) -> Result<JsonValue, CoreApiError> {
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
    pub async fn high_water_mark() -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_conversation_high_water_mark(state)?)
        })
        .await
    }
}
