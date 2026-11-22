//! Conversation commands that need a channel account or the network: stickers, recall,
//! reactions, forwarding, attachments, sync, WhatsApp login and group members.

use crate::error::CoreApiError;
use crate::runtime::{
    body, checked_id64, checked_ids32, checked_ids64, checked_optional_id64, id64, limit,
    on_blocking, on_core, state_of,
};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::commands::{self, BatchImportGroupMembersRequest, DbState};
use rustsale_core::conversation::commands::{self as conversation, ConversationState};
use rustsale_core::conversation::types::{
    ForwardConversationMessageRequest, RecallMessageRequest, RetryConversationForwardTargetRequest,
    SetConversationReactionRequest, StageConversationClipboardImageRequest,
    StartDirectConversationRequest,
};

/// A JS number as a sticker id, refusing values a `u32` cannot hold.
fn sticker_id(value: f64) -> Result<u32, CoreApiError> {
    if value.is_finite() && value.fract() == 0.0 && value >= 0.0 && value <= f64::from(u32::MAX) {
        Ok(value as u32)
    } else {
        Err(CoreApiError::invalid("sticker id is out of range"))
    }
}

/// Conversation actions bound to a channel account: stickers, recall, reactions, forwarding,
/// attachments, history sync, WhatsApp QR and group members. Sending and sync commands need a
/// connected account; their errors carry the core's conversation code.
#[module]
pub struct ConversationNetwork;

#[module]
impl ConversationNetwork {
    /// Restarts the Zalo listener of account `providerAccountKey`.
    pub async fn restart_account(provider_account_key: String) -> Result<JsonValue, JsError> {
        on_core(move |h| async move {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(
                conversation::restart_conversation_account(state, provider_account_key).await?,
            )
        })
        .await
    }

    /// Opens a direct conversation from `request` (`{ channelAccountId, phoneOrExternalId,
    /// displayName?, initialMessage }`) and resolves its first message.
    pub async fn start_direct(request: JsonValue) -> Result<JsonValue, JsError> {
        let mut request: StartDirectConversationRequest = body(request)?;
        request.channel_account_id = checked_id64(request.channel_account_id)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::start_direct_conversation(state, request)?)
        })
        .await
    }

    /// Copies the files at `paths` into the staging area of conversation `conversationId` and
    /// resolves their metadata.
    pub async fn stage_attachment_paths(
        conversation_id: f64,
        paths: Vec<String>,
    ) -> Result<JsonValue, JsError> {
        let conversation = id64(conversation_id)?;
        on_core(move |h| async move {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(
                conversation::stage_conversation_attachment_paths(state, conversation, paths)
                    .await?,
            )
        })
        .await
    }

    /// Stages a clipboard image from `request` (`{ conversationId, fileName, mimeType,
    /// base64Data }`) and resolves its metadata.
    pub async fn stage_clipboard_image(request: JsonValue) -> Result<JsonValue, JsError> {
        let mut request: StageConversationClipboardImageRequest = body(request)?;
        request.conversation_id = checked_id64(request.conversation_id)?;
        on_core(move |h| async move {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(
                conversation::stage_conversation_clipboard_image(state, request).await?,
            )
        })
        .await
    }

    /// Discards staged attachment `stagedAttachmentId` and deletes its staged file.
    pub async fn discard_staged_attachment(
        staged_attachment_id: String,
    ) -> Result<JsonValue, JsError> {
        on_core(move |h| async move {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(
                conversation::discard_conversation_staged_attachment(state, staged_attachment_id)
                    .await?,
            )
        })
        .await
    }

    /// Searches Zalo stickers for conversation `conversationId`; `limit` absent means 20.
    pub async fn search_stickers(
        conversation_id: f64,
        query: String,
        limit_: Option<f64>,
    ) -> Result<JsonValue, JsError> {
        let conversation = id64(conversation_id)?;
        let size = limit(limit_);
        on_core(move |h| async move {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(
                conversation::search_zalo_stickers(state, conversation, query, size).await?,
            )
        })
        .await
    }

    /// One Zalo sticker by id, as the chat of conversation `conversationId` sees it; `null` when
    /// Zalo has no such sticker.
    pub async fn sticker_detail(conversation_id: f64, sticker: f64) -> Result<JsonValue, JsError> {
        let conversation = id64(conversation_id)?;
        let sticker = sticker_id(sticker)?;
        on_core(move |h| async move {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(
                conversation::get_zalo_sticker_detail(state, conversation, sticker).await?,
            )
        })
        .await
    }

    /// Recalls sent message `request` (`{ messageId }`) and resolves the updated message.
    pub async fn recall_message(request: JsonValue) -> Result<JsonValue, JsError> {
        let mut request: RecallMessageRequest = body(request)?;
        request.message_id = checked_id64(request.message_id)?;
        on_core(move |h| async move {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::recall_conversation_message(state, request).await?)
        })
        .await
    }

    /// Adds or removes reaction `request` (`{ messageId, reaction, active, count?,
    /// clientMutationId }`) on a message; resolves the mutation result.
    pub async fn set_reaction(request: JsonValue) -> Result<JsonValue, JsError> {
        let mut request: SetConversationReactionRequest = body(request)?;
        request.message_id = checked_id64(request.message_id)?;
        on_core(move |h| async move {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::set_conversation_reaction(state, request).await?)
        })
        .await
    }

    /// Forwards message `request` (`{ clientOperationId, sourceMessageId,
    /// targetConversationIds }`) to several conversations; resolves the operation with per-target
    /// results.
    pub async fn forward_message(request: JsonValue) -> Result<JsonValue, JsError> {
        let mut request: ForwardConversationMessageRequest = body(request)?;
        request.source_message_id = checked_id64(request.source_message_id)?;
        request.target_conversation_ids = checked_ids64(request.target_conversation_ids)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::forward_conversation_message(state, request)?)
        })
        .await
    }

    /// Retries one failed target of a forward from `request` (`{ clientOperationId,
    /// targetConversationId }`) and resolves that target's result.
    pub async fn retry_forward_target(request: JsonValue) -> Result<JsonValue, JsError> {
        let mut request: RetryConversationForwardTargetRequest = body(request)?;
        request.target_conversation_id = checked_id64(request.target_conversation_id)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::retry_conversation_forward_target(
                state, request,
            )?)
        })
        .await
    }

    /// The members of group conversation `conversationId` with their lead and categories. The
    /// name keeps the v2 `unrestricted` meaning: the group may have more than the page shows.
    pub async fn group_members(conversation_id: f64) -> Result<JsonValue, JsError> {
        let conversation = id64(conversation_id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_group_members_unrestricted(
                state,
                conversation,
            )?)
        })
        .await
    }

    /// Imports group members from `request` (`{ conversationId, members, categoryIds,
    /// newCategoryName?, groupTitle, skipExisting? }`) as leads and resolves the counts.
    pub async fn import_group_members(request: JsonValue) -> Result<JsonValue, JsError> {
        let mut request: BatchImportGroupMembersRequest = body(request)?;
        request.conversation_id = checked_id64(request.conversation_id)?;
        request.category_ids = checked_ids32(request.category_ids)?;
        for member in &mut request.members {
            member.identity_id = checked_optional_id64(member.identity_id)?;
        }
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::batch_import_group_members_to_leads(
                state, request,
            )?)
        })
        .await
    }

    /// Pulls the member profiles of group `conversationId` from the channel runtime and resolves
    /// the scan result.
    pub async fn deep_scan_group_members(conversation_id: f64) -> Result<JsonValue, JsError> {
        let conversation = id64(conversation_id)?;
        on_core(move |h| async move {
            let db = state_of::<DbState>(&h)?;
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(
                commands::sync_and_deep_scan_group_members(db, state, conversation).await?,
            )
        })
        .await
    }

    /// Backfills up to five pages of the Zalo group history of `conversationId`; resolves how many
    /// messages were added.
    pub async fn sync_zalo_group_history(conversation_id: f64) -> Result<JsonValue, JsError> {
        let conversation = id64(conversation_id)?;
        on_core(move |h| async move {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(
                conversation::sync_zalo_group_chat_history(state, conversation).await?,
            )
        })
        .await
    }

    /// Syncs WhatsApp conversations from every connected account; resolves how many were touched.
    pub async fn sync_whatsapp_conversations() -> Result<JsonValue, JsError> {
        on_core(move |h| async move {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::sync_whatsapp_conversations(state).await?)
        })
        .await
    }

    /// Downloads attachment `attachmentId` into the local spool and resolves its record.
    pub async fn download_attachment(attachment_id: f64) -> Result<JsonValue, JsError> {
        let attachment = id64(attachment_id)?;
        on_core(move |h| async move {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(
                conversation::download_conversation_attachment(state, attachment).await?,
            )
        })
        .await
    }

    /// Starts the WhatsApp QR login attempt `attemptId`; the QR and the result arrive as
    /// `whatsapp-qr-code` and `whatsapp-login-*` events on `Host.onEvent`.
    pub async fn start_whatsapp_qr(attempt_id: String) -> Result<JsonValue, JsError> {
        on_core(move |h| async move {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::start_whatsapp_qr_login(state, attempt_id).await?)
        })
        .await
    }

    /// Cancels WhatsApp QR login attempt `attemptId` and removes its temporary session.
    pub async fn cancel_whatsapp_qr(attempt_id: String) -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::cancel_whatsapp_qr_login(state, attempt_id)?)
        })
        .await
    }

    /// Connected WhatsApp accounts with their phone number and push name.
    pub async fn whatsapp_accounts() -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::get_managed_whatsapp_accounts(state)?)
        })
        .await
    }

    /// Disconnects WhatsApp account `channelAccountId`: it stops the bot and deletes the session.
    pub async fn disconnect_whatsapp_account(
        channel_account_id: f64,
    ) -> Result<JsonValue, JsError> {
        let account = id64(channel_account_id)?;
        on_core(move |h| async move {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::disconnect_whatsapp_account(state, account).await?)
        })
        .await
    }
}
