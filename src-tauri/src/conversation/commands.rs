use super::error::{ConversationCommandError, ConversationError, ConversationErrorCode};
use super::service::ConversationService;
use super::types::{
    Channel, ConversationForwardTargetResult, ConversationMentionCandidate, ConversationMessage,
    ConversationQuickTemplate, ConversationReactionMutationResult, ConversationRuntimeStatus,
    CreateConversationQuickTemplateRequest, DeleteConversationQuickTemplateRequest,
    DeleteConversationRequest, ForwardConversationMessageRequest, ForwardConversationMessageResult,
    InboxItem, LinkIdentityRequest, ListConversationMentionCandidatesRequest,
    ListConversationQuickTemplatesRequest, MarkConversationQuickTemplateUsedRequest,
    PinMessageRequest, PinnedMessage, RelinkLeadRequest, RetryConversationForwardTargetRequest,
    SendConversationBatchRequest, SendConversationBatchResult, SendConversationMessageRequest,
    SetConversationReactionRequest, StageConversationClipboardImageRequest,
    StagedAttachmentResponse, StartDirectConversationRequest, TimelineCursor, TimelinePage,
    UnlinkIdentityRequest, UnlinkLeadRequest, UnpinMessageRequest,
    UpdateConversationQuickTemplateRequest,
};

use super::facebook::supervisor::FacebookMessengerSupervisor;
use super::whatsapp::login::WhatsAppLoginManager;
use super::whatsapp::supervisor::WhatsAppSupervisor;
use super::zalo::supervisor::ZaloSessionSupervisor;
use std::sync::Arc;

pub struct ConversationState {
    pub service: Arc<ConversationService>,
    pub zalo_supervisor: Arc<ZaloSessionSupervisor>,
    pub facebook_supervisor: Arc<FacebookMessengerSupervisor>,
    pub whatsapp_supervisor: Arc<WhatsAppSupervisor>,
    pub whatsapp_login_manager: Arc<WhatsAppLoginManager>,
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
pub fn mark_conversation_read(
    state: tauri::State<'_, ConversationState>,
    key: String,
) -> Result<u64, String> {
    state
        .service
        .mark_conversation_read(&key)
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
        .service
        .runtime_status()
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
pub fn delete_conversations_batch(
    state: tauri::State<'_, ConversationState>,
    requests: Vec<DeleteConversationRequest>,
) -> Result<(), String> {
    state
        .service
        .delete_conversations_batch(&requests)
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
    // Boundary validation: the only Messenger 24h-policy tag this build
    // accepts is HUMAN_AGENT (legacy tags are retired by Meta). Reject
    // arbitrary strings at the IPC boundary instead of silently dropping.
    if let Some(tag) = &request.message_tag {
        if tag != "HUMAN_AGENT" {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidRequest,
                format!("unsupported message_tag: {tag}"),
            )
            .command_error());
        }
    }
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
pub async fn get_zalo_sticker_detail(
    state: tauri::State<'_, ConversationState>,
    conversation_id: i64,
    sticker_id: u32,
) -> Result<Option<crate::conversation::zalo::client::ZaloStickerReference>, ConversationCommandError>
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
        .get_sticker_detail(&account_key, sticker_id)
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

#[tauri::command]
pub fn list_conversation_quick_templates(
    state: tauri::State<'_, ConversationState>,
    request: ListConversationQuickTemplatesRequest,
) -> Result<Vec<ConversationQuickTemplate>, ConversationCommandError> {
    state
        .service
        .list_quick_templates(request)
        .map_err(|error| error.command_error())
}

#[tauri::command]
pub fn create_conversation_quick_template(
    state: tauri::State<'_, ConversationState>,
    request: CreateConversationQuickTemplateRequest,
) -> Result<ConversationQuickTemplate, ConversationCommandError> {
    state
        .service
        .create_quick_template(request)
        .map_err(|error| error.command_error())
}

#[tauri::command]
pub fn update_conversation_quick_template(
    state: tauri::State<'_, ConversationState>,
    request: UpdateConversationQuickTemplateRequest,
) -> Result<ConversationQuickTemplate, ConversationCommandError> {
    state
        .service
        .update_quick_template(request)
        .map_err(|error| error.command_error())
}

#[tauri::command]
pub fn delete_conversation_quick_template(
    state: tauri::State<'_, ConversationState>,
    request: DeleteConversationQuickTemplateRequest,
) -> Result<(), ConversationCommandError> {
    state
        .service
        .delete_quick_template(request)
        .map_err(|error| error.command_error())
}

#[tauri::command]
pub fn mark_conversation_quick_template_used(
    state: tauri::State<'_, ConversationState>,
    request: MarkConversationQuickTemplateUsedRequest,
) -> Result<(), ConversationCommandError> {
    state
        .service
        .mark_quick_template_used(request)
        .map_err(|error| error.command_error())
}

#[tauri::command]
pub async fn set_conversation_reaction(
    state: tauri::State<'_, ConversationState>,
    request: SetConversationReactionRequest,
) -> Result<ConversationReactionMutationResult, ConversationCommandError> {
    use crate::conversation::error::{ConversationError, ConversationErrorCode};

    let (provider_account_key, ..) = state
        .service
        .repository
        .lookup_message_thread_info(request.message_id)
        .map_err(|e| ConversationError::from(e).command_error())?;

    let client = state
        .zalo_supervisor
        .connected_client(&provider_account_key)
        .await
        .ok_or_else(|| {
            ConversationError::new(
                ConversationErrorCode::AccountDisconnected,
                format!("Zalo account {provider_account_key} is disconnected"),
            )
            .command_error()
        })?;

    state
        .service
        .set_conversation_reaction(&request, client.as_ref())
        .await
        .map_err(|e| e.command_error())
}

#[tauri::command]
pub fn forward_conversation_message(
    state: tauri::State<'_, ConversationState>,
    request: ForwardConversationMessageRequest,
) -> Result<ForwardConversationMessageResult, ConversationCommandError> {
    state
        .service
        .forward_conversation_message(request)
        .map_err(|error| error.command_error())
}

#[tauri::command]
pub fn get_conversation_forward_operation(
    state: tauri::State<'_, ConversationState>,
    client_operation_id: String,
) -> Result<ForwardConversationMessageResult, ConversationCommandError> {
    state
        .service
        .get_forward_operation(&client_operation_id)
        .map_err(|error| error.command_error())
}

#[tauri::command]
pub fn retry_conversation_forward_target(
    state: tauri::State<'_, ConversationState>,
    request: RetryConversationForwardTargetRequest,
) -> Result<ConversationForwardTargetResult, ConversationCommandError> {
    state
        .service
        .retry_forward_target(&request.client_operation_id, request.target_conversation_id)
        .map_err(|error| error.command_error())
}

#[tauri::command]
pub fn list_conversation_mention_candidates(
    state: tauri::State<'_, ConversationState>,
    request: ListConversationMentionCandidatesRequest,
) -> Result<Vec<ConversationMentionCandidate>, ConversationCommandError> {
    state
        .service
        .list_mention_candidates(
            request.conversation_id,
            request.source_message_id,
            request.query.as_deref(),
            request.limit,
        )
        .map_err(|error| error.command_error())
}

#[tauri::command]
pub fn pin_conversation_message(
    state: tauri::State<'_, ConversationState>,
    request: PinMessageRequest,
) -> Result<PinnedMessage, String> {
    state
        .service
        .pin_message(
            request.conversation_id,
            request.message_id,
            request.pinned_by.as_deref(),
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn unpin_conversation_message(
    state: tauri::State<'_, ConversationState>,
    request: UnpinMessageRequest,
) -> Result<(), String> {
    state
        .service
        .unpin_message(request.conversation_id, request.message_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_conversation_pinned_messages(
    state: tauri::State<'_, ConversationState>,
    conversation_id: i64,
) -> Result<Vec<PinnedMessage>, String> {
    state
        .service
        .list_pinned_messages(conversation_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn sync_facebook_conversations(
    state: tauri::State<'_, ConversationState>,
    page_id: Option<String>,
) -> Result<usize, String> {
    match page_id {
        Some(id) if !id.trim().is_empty() => state
            .facebook_supervisor
            .sync_page(id.trim())
            .await
            .map_err(|e| e.to_string()),
        _ => state
            .facebook_supervisor
            .sync_all()
            .await
            .map_err(|e| e.to_string()),
    }
}

#[tauri::command]
pub fn ingest_facebook_webhook_events(
    state: tauri::State<'_, ConversationState>,
    page_id: String,
    events: Vec<serde_json::Value>,
) -> Result<usize, String> {
    state
        .facebook_supervisor
        .ingest_webhook_events(page_id.trim(), &events)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_whatsapp_qr_login(
    state: tauri::State<'_, ConversationState>,
    attempt_id: String,
) -> Result<(), String> {
    state.whatsapp_login_manager.start_qr_login(&attempt_id).await
}

#[tauri::command]
pub fn cancel_whatsapp_qr_login(
    state: tauri::State<'_, ConversationState>,
    attempt_id: String,
) -> Result<(), String> {
    state.whatsapp_login_manager.cancel_qr_login(&attempt_id)
}

#[tauri::command]
pub fn get_managed_whatsapp_accounts(
    state: tauri::State<'_, ConversationState>,
) -> Result<Vec<crate::conversation::whatsapp::supervisor::ConnectedWhatsAppAccount>, String> {
    let conn = crate::db::open_db_connection(state.service.repository.db_path())
        .map_err(|e| e.to_string())?;
    state
        .whatsapp_supervisor
        .get_connected_accounts(&conn)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn disconnect_whatsapp_account(
    state: tauri::State<'_, ConversationState>,
    channel_account_id: i64,
) -> Result<(), String> {
    state
        .whatsapp_supervisor
        .unregister_client(channel_account_id)
        .await;
    let conn = crate::db::open_db_connection(state.service.repository.db_path())
        .map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE channel_accounts 
         SET connection_status = 'disconnected', enabled = 0, updated_at = ?1 
         WHERE id = ?2 AND channel = 'whatsapp'",
        rusqlite::params![now, channel_account_id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE whatsapp_sessions 
         SET is_valid = 0, updated_at = ?1 
         WHERE channel_account_id = ?2",
        rusqlite::params![now, channel_account_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn create_whatsapp_campaign(
    state: tauri::State<'_, ConversationState>,
    req: crate::conversation::whatsapp::campaign::CreateCampaignRequest,
) -> Result<i64, String> {
    let conn = crate::db::open_db_connection(state.service.repository.db_path())
        .map_err(|e| e.to_string())?;
    crate::conversation::whatsapp::campaign::create_campaign(&conn, &req)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn populate_whatsapp_campaign_recipients(
    state: tauri::State<'_, ConversationState>,
    campaign_id: i64,
    recipients: Vec<crate::conversation::whatsapp::campaign::CampaignRecipientInput>,
    template: Option<String>,
) -> Result<usize, String> {
    let conn = crate::db::open_db_connection(state.service.repository.db_path())
        .map_err(|e| e.to_string())?;
    crate::conversation::whatsapp::campaign::populate_recipients(
        &conn,
        campaign_id,
        &recipients,
        template.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn start_whatsapp_campaign(
    state: tauri::State<'_, ConversationState>,
    campaign_id: i64,
) -> Result<(), String> {
    let conn = crate::db::open_db_connection(state.service.repository.db_path())
        .map_err(|e| e.to_string())?;
    crate::conversation::whatsapp::campaign::start_campaign(&conn, campaign_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pause_whatsapp_campaign(
    state: tauri::State<'_, ConversationState>,
    campaign_id: i64,
) -> Result<(), String> {
    let conn = crate::db::open_db_connection(state.service.repository.db_path())
        .map_err(|e| e.to_string())?;
    crate::conversation::whatsapp::campaign::pause_campaign(&conn, campaign_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resume_whatsapp_campaign(
    state: tauri::State<'_, ConversationState>,
    campaign_id: i64,
) -> Result<(), String> {
    let conn = crate::db::open_db_connection(state.service.repository.db_path())
        .map_err(|e| e.to_string())?;
    crate::conversation::whatsapp::campaign::resume_campaign(&conn, campaign_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cancel_whatsapp_campaign(
    state: tauri::State<'_, ConversationState>,
    campaign_id: i64,
) -> Result<(), String> {
    let conn = crate::db::open_db_connection(state.service.repository.db_path())
        .map_err(|e| e.to_string())?;
    crate::conversation::whatsapp::campaign::cancel_campaign(&conn, campaign_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_whatsapp_campaign(
    state: tauri::State<'_, ConversationState>,
    campaign_id: i64,
) -> Result<crate::conversation::whatsapp::campaign::WhatsAppCampaignRecord, String> {
    let conn = crate::db::open_db_connection(state.service.repository.db_path())
        .map_err(|e| e.to_string())?;
    crate::conversation::whatsapp::campaign::get_campaign(&conn, campaign_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_whatsapp_campaigns(
    state: tauri::State<'_, ConversationState>,
) -> Result<Vec<crate::conversation::whatsapp::campaign::WhatsAppCampaignRecord>, String> {
    let conn = crate::db::open_db_connection(state.service.repository.db_path())
        .map_err(|e| e.to_string())?;
    crate::conversation::whatsapp::campaign::list_campaigns(&conn)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_whatsapp_campaign_recipients(
    state: tauri::State<'_, ConversationState>,
    campaign_id: i64,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<crate::conversation::whatsapp::campaign::CampaignRecipientRecord>, String> {
    let conn = crate::db::open_db_connection(state.service.repository.db_path())
        .map_err(|e| e.to_string())?;
    crate::conversation::whatsapp::campaign::get_campaign_recipients(
        &conn,
        campaign_id,
        limit,
        offset,
    )
    .map_err(|e| e.to_string())
}
