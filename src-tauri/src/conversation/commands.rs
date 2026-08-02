use super::error::ConversationCommandError;
use super::service::ConversationService;
use super::types::{
    Channel, ConversationMessage, ConversationRuntimeStatus, DeleteConversationRequest, InboxItem,
    LinkIdentityRequest, RelinkLeadRequest, SendConversationBatchRequest,
    SendConversationBatchResult, SendConversationMessageRequest,
    StageConversationClipboardImageRequest, StagedAttachmentResponse,
    StartDirectConversationRequest, TimelineCursor, TimelinePage, UnlinkIdentityRequest,
    UnlinkLeadRequest,
};
use super::zalo::supervisor::ZaloSessionSupervisor;
use std::sync::Arc;

pub struct ConversationState {
    pub service: Arc<ConversationService>,
    pub zalo_supervisor: Arc<ZaloSessionSupervisor>,
}

#[tauri::command]
pub fn list_conversation_inbox(
    state: tauri::State<'_, ConversationState>,
) -> Result<Vec<InboxItem>, String> {
    state
        .service
        .list_inbox()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_contact_timeline(
    state: tauri::State<'_, ConversationState>,
    lead_id: i64,
    active_channels: Vec<Channel>,
    before: Option<TimelineCursor>,
    limit: u32,
) -> Result<TimelinePage, String> {
    state
        .service
        .get_contact_timeline(lead_id, &active_channels, before.as_ref(), limit)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_conversation_timeline(
    state: tauri::State<'_, ConversationState>,
    conversation_id: i64,
    before: Option<TimelineCursor>,
    limit: u32,
) -> Result<TimelinePage, String> {
    state
        .service
        .get_conversation_timeline(conversation_id, before.as_ref(), limit)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_identity_timeline(
    state: tauri::State<'_, ConversationState>,
    identity_id: i64,
    before: Option<TimelineCursor>,
    limit: u32,
) -> Result<TimelinePage, String> {
    state
        .service
        .get_identity_timeline(identity_id, before.as_ref(), limit)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn link_conversation_identity(
    state: tauri::State<'_, ConversationState>,
    request: LinkIdentityRequest,
) -> Result<(), String> {
    state
        .service
        .link_identity(request.identity_id, request.lead_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn unlink_conversation_identity(
    state: tauri::State<'_, ConversationState>,
    request: UnlinkIdentityRequest,
) -> Result<(), String> {
    state
        .service
        .unlink_identity(request.identity_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn unlink_conversation_lead(
    state: tauri::State<'_, ConversationState>,
    request: UnlinkLeadRequest,
) -> Result<(), String> {
    state
        .service
        .unlink_lead(request.lead_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn relink_conversation_lead(
    state: tauri::State<'_, ConversationState>,
    request: RelinkLeadRequest,
) -> Result<(), String> {
    state
        .service
        .relink_lead(request.source_lead_id, request.target_lead_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn send_conversation_message(
    state: tauri::State<'_, ConversationState>,
    request: SendConversationMessageRequest,
) -> Result<ConversationMessage, String> {
    state
        .service
        .enqueue_message(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_conversation_runtime_status(
    state: tauri::State<'_, ConversationState>,
) -> Result<ConversationRuntimeStatus, String> {
    state
        .zalo_supervisor
        .runtime_status()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn restart_conversation_account(
    state: tauri::State<'_, ConversationState>,
    provider_account_key: String,
) -> Result<(), String> {
    state
        .zalo_supervisor
        .restart_account(&provider_account_key)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_conversation(
    state: tauri::State<'_, ConversationState>,
    request: DeleteConversationRequest,
) -> Result<(), String> {
    state
        .service
        .delete_conversation(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn start_direct_conversation(
    state: tauri::State<'_, ConversationState>,
    request: StartDirectConversationRequest,
) -> Result<ConversationMessage, String> {
    state
        .service
        .start_direct_conversation(&request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn send_conversation_batch(
    state: tauri::State<'_, ConversationState>,
    request: SendConversationBatchRequest,
) -> Result<SendConversationBatchResult, ConversationCommandError> {
    state
        .service
        .enqueue_batch(&request)
        .map_err(|error| error.command_error())
}

#[tauri::command]
pub async fn stage_conversation_attachment_paths(
    state: tauri::State<'_, ConversationState>,
    conversation_id: i64,
    paths: Vec<String>,
) -> Result<Vec<StagedAttachmentResponse>, ConversationCommandError> {
    state
        .service
        .stager
        .stage_paths(conversation_id, paths)
        .await
        .map_err(|error| error.command_error())
}

#[tauri::command]
pub async fn stage_conversation_clipboard_image(
    state: tauri::State<'_, ConversationState>,
    request: StageConversationClipboardImageRequest,
) -> Result<StagedAttachmentResponse, ConversationCommandError> {
    state
        .service
        .stager
        .stage_clipboard_image(request)
        .await
        .map_err(|error| error.command_error())
}

#[tauri::command]
pub async fn discard_conversation_staged_attachment(
    state: tauri::State<'_, ConversationState>,
    staged_attachment_id: String,
) -> Result<(), ConversationCommandError> {
    state
        .service
        .stager
        .discard(&staged_attachment_id)
        .await
        .map_err(|error| error.command_error())
}

#[tauri::command]
pub async fn search_zalo_stickers(
    state: tauri::State<'_, ConversationState>,
    conversation_id: i64,
    query: String,
    limit: u32,
) -> Result<Vec<crate::conversation::zalo::client::ZaloStickerReference>, ConversationCommandError>
{
    use crate::conversation::error::{ConversationError, ConversationErrorCode};

    let account_key = state
        .service
        .repository
        .get_conversation_account_key(conversation_id)
        .map_err(|e| ConversationError::from(e).command_error())?
        .ok_or_else(|| {
            ConversationError::new(
                ConversationErrorCode::InvalidRequest,
                format!("conversation {conversation_id} has no channel account"),
            )
            .command_error()
        })?;

    state
        .zalo_supervisor
        .search_stickers(&account_key, &query, limit)
        .await
        .map_err(|e| {
            ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
                .command_error()
        })
}

#[tauri::command]
pub async fn recall_conversation_message(
    state: tauri::State<'_, ConversationState>,
    request: crate::conversation::types::RecallMessageRequest,
) -> Result<ConversationMessage, ConversationCommandError> {
    use crate::conversation::error::{ConversationError, ConversationErrorCode};

    let prepared = state
        .service
        .prepare_recall(request.message_id)
        .map_err(|e| ConversationError::from(e).command_error())?;

    let client = state
        .zalo_supervisor
        .connected_client(&prepared.provider_account_key)
        .await
        .ok_or_else(|| {
            ConversationError::new(
                ConversationErrorCode::ProviderAuth,
                format!(
                    "Zalo account {} is disconnected",
                    prepared.provider_account_key
                ),
            )
            .command_error()
        })?;

    let thread_type = if prepared.is_group {
        zca_rs::models::ThreadType::Group
    } else {
        zca_rs::models::ThreadType::User
    };

    let recall_req = crate::conversation::zalo::client::ProviderRecallRequest {
        external_thread_id: prepared.external_thread_id,
        thread_type,
        provider_message_id: prepared.provider_message_id,
        provider_client_message_id: prepared.provider_client_message_id,
    };

    client.recall(recall_req).await.map_err(|e| {
        ConversationError::new(ConversationErrorCode::ProviderFailure, e.message).command_error()
    })?;

    let now = crate::conversation::types::utc_now();
    state
        .service
        .complete_recall(prepared.message_id, &now)
        .map_err(|e| ConversationError::from(e).command_error())
}
