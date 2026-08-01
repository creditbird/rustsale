use super::service::ConversationService;
use super::types::{
    Channel, ConversationMessage, ConversationRuntimeStatus, InboxItem, LinkIdentityRequest,
    SendConversationMessageRequest, TimelineCursor, TimelinePage, UnlinkIdentityRequest,
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
