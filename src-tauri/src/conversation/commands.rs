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
    let msg = state
        .service
        .enqueue_message(&request)
        .map_err(|error| error.to_string())?;
    state.zalo_supervisor.notify_outbox();
    state.whatsapp_supervisor.notify_outbox();
    Ok(msg)
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
    let msg = state
        .service
        .start_direct_conversation(&request)
        .map_err(|error| error.to_string())?;
    state.zalo_supervisor.notify_outbox();
    state.whatsapp_supervisor.notify_outbox();
    Ok(msg)
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
    let result = state
        .service
        .enqueue_batch(&request)
        .map_err(|error| error.command_error())?;
    state.zalo_supervisor.notify_outbox();
    state.whatsapp_supervisor.notify_outbox();
    Ok(result)
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

    let msg = state
        .service
        .repository
        .get_message_by_id(request.message_id)
        .map_err(|e| ConversationError::from(e).command_error())?
        .ok_or_else(|| {
            ConversationError::new(
                ConversationErrorCode::InvalidRequest,
                format!("Message {} not found", request.message_id),
            )
            .command_error()
        })?;

    match msg.channel {
        Channel::Zalo => {
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
        Channel::WhatsApp => {
            let client = state
                .whatsapp_supervisor
                .get_client(msg.channel_account_id)
                .await
                .ok_or_else(|| {
                    ConversationError::new(
                        ConversationErrorCode::AccountDisconnected,
                        format!(
                            "WhatsApp account {} is disconnected",
                            msg.channel_account_id
                        ),
                    )
                    .command_error()
                })?;

            state
                .service
                .set_whatsapp_conversation_reaction(&request, client.as_ref())
                .await
                .map_err(|e| e.command_error())
        }
        _ => Err(ConversationError::new(
            ConversationErrorCode::ReactionNotSupported,
            format!(
                "Reactions not supported for channel {}",
                msg.channel.as_str()
            ),
        )
        .command_error()),
    }
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
    state
        .whatsapp_login_manager
        .start_qr_login(&attempt_id)
        .await
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
        .disconnect_account(channel_account_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_whatsapp_campaign(
    state: tauri::State<'_, ConversationState>,
    req: crate::conversation::whatsapp::campaign::CreateCampaignRequest,
) -> Result<i64, String> {
    let conn = crate::db::open_db_connection(state.service.repository.db_path())
        .map_err(|e| e.to_string())?;
    crate::conversation::whatsapp::campaign::create_campaign(&conn, &req).map_err(|e| e.to_string())
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
pub async fn start_whatsapp_campaign(
    state: tauri::State<'_, ConversationState>,
    campaign_id: i64,
) -> Result<(), String> {
    let conn = crate::db::open_db_connection(state.service.repository.db_path())
        .map_err(|e| e.to_string())?;
    crate::conversation::whatsapp::campaign::start_campaign(&conn, campaign_id)
        .map_err(|e| e.to_string())?;

    let db_path = state.service.repository.db_path().to_string();
    let supervisor = state.whatsapp_supervisor.clone();
    let cancel_token = supervisor.cancellation_token();
    tokio::spawn(async move {
        crate::conversation::whatsapp::campaign::run_campaign_worker(
            db_path,
            campaign_id,
            supervisor,
            cancel_token,
        )
        .await;
    });

    Ok(())
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
    crate::conversation::whatsapp::campaign::list_campaigns(&conn).map_err(|e| e.to_string())
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

#[tauri::command]
pub async fn sync_zalo_group_chat_history(
    state: tauri::State<'_, ConversationState>,
    conversation_id: i64,
) -> Result<usize, String> {
    state
        .zalo_supervisor
        .sync_group_chat_history(conversation_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn sync_whatsapp_conversations(
    state: tauri::State<'_, ConversationState>,
) -> Result<usize, String> {
    let loaded = state
        .whatsapp_supervisor
        .load_active_sessions_on_startup()
        .await
        .map_err(|e| e.to_string())?;
    state
        .whatsapp_supervisor
        .drain_all_connected_accounts()
        .await;
    Ok(loaded)
}

const MAX_ATTACHMENT_PREVIEW_BYTES: u64 = super::staging::MAX_ATTACHMENT_BYTES;
const WHATSAPP_MEDIA_SECRET_BYTES: usize = 32;

fn media_metadata_unavailable(message: impl Into<String>) -> ConversationCommandError {
    ConversationError::new(ConversationErrorCode::MediaMetadataUnavailable, message).command_error()
}

fn ensure_attachment_preview_size(size_bytes: u64) -> Result<(), ConversationCommandError> {
    if size_bytes > MAX_ATTACHMENT_PREVIEW_BYTES {
        return Err(ConversationError::new(
            ConversationErrorCode::FileTooLarge,
            "Ảnh vượt quá giới hạn xem trước 50 MiB",
        )
        .command_error());
    }
    Ok(())
}

fn attachment_preview_is_resolved(metadata_json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(metadata_json)
        .ok()
        .and_then(|metadata| metadata.get("preview_resolved").cloned())
        .map(|value| value.as_bool().unwrap_or_else(|| value.as_i64() == Some(1)))
        .unwrap_or(false)
}

fn browser_renderable_thumbnail(value: &str) -> bool {
    value.starts_with("data:image/")
        || (!value.to_ascii_lowercase().contains("mmg.whatsapp.net")
            && (value.starts_with("http://")
                || value.starts_with("https://")
                || value.starts_with("asset://")))
}

fn base64_data_url_decoded_size(value: &str) -> Option<u64> {
    let (header, payload) = value.split_once(',')?;
    if !header.to_ascii_lowercase().ends_with(";base64") || payload.is_empty() {
        return None;
    }
    let payload_len = u64::try_from(payload.len()).ok()?;
    if payload_len % 4 != 0 {
        return None;
    }
    let padding = payload
        .as_bytes()
        .iter()
        .rev()
        .take_while(|byte| **byte == b'=')
        .take(2)
        .count() as u64;
    payload_len
        .checked_div(4)?
        .checked_mul(3)?
        .checked_sub(padding)
}

fn whatsapp_reference_is_encrypted(value: Option<&str>) -> bool {
    let normalized = value.unwrap_or_default().trim().to_ascii_lowercase();
    normalized.contains("mmg.whatsapp.net")
        || normalized.starts_with("/o1/")
        || normalized.starts_with("/v/t62")
        || normalized.starts_with("/mms/")
        || normalized
            .split(['?', '#'])
            .next()
            .is_some_and(|path| path.ends_with(".enc"))
}

fn whatsapp_media_key_available(metadata_json: &str, raw_payload_json: Option<&str>) -> bool {
    let has_key = |value: &serde_json::Value| {
        value
            .get("media_key")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|key| !key.trim().is_empty())
    };
    if serde_json::from_str::<serde_json::Value>(metadata_json)
        .ok()
        .as_ref()
        .is_some_and(has_key)
    {
        return true;
    }
    raw_payload_json
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .and_then(|raw| {
            raw.get("attachments")
                .and_then(serde_json::Value::as_array)
                .cloned()
        })
        .is_some_and(|attachments| attachments.iter().any(has_key))
}

async fn read_attachment_file_bounded(
    path: &std::path::Path,
) -> Result<Vec<u8>, ConversationCommandError> {
    use tokio::io::AsyncReadExt as _;

    let metadata = tokio::fs::symlink_metadata(path).await.map_err(|error| {
        ConversationError::new(
            ConversationErrorCode::ProviderFailure,
            format!("Không thể đọc thông tin file ảnh đã lưu: {error}"),
        )
        .command_error()
    })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(ConversationError::new(
            ConversationErrorCode::UnsupportedMedia,
            "Đường dẫn ảnh đã lưu không phải file thông thường",
        )
        .command_error());
    }
    ensure_attachment_preview_size(metadata.len())?;

    let file = tokio::fs::File::open(path).await.map_err(|error| {
        ConversationError::new(
            ConversationErrorCode::ProviderFailure,
            format!("Không thể mở file ảnh đã lưu: {error}"),
        )
        .command_error()
    })?;
    let mut limited_reader = file.take(MAX_ATTACHMENT_PREVIEW_BYTES.saturating_add(1));
    let mut bytes = Vec::with_capacity(metadata.len().min(MAX_ATTACHMENT_PREVIEW_BYTES) as usize);
    limited_reader
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| {
            ConversationError::new(
                ConversationErrorCode::ProviderFailure,
                format!("Không thể đọc file ảnh đã lưu: {error}"),
            )
            .command_error()
        })?;
    ensure_attachment_preview_size(bytes.len() as u64)?;
    Ok(bytes)
}

fn decode_required_whatsapp_media_key(
    encoded: Option<&str>,
) -> Result<Vec<u8>, ConversationCommandError> {
    use base64::Engine as _;

    let encoded = encoded
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            media_metadata_unavailable(
                "Ảnh lịch sử thiếu khóa giải mã E2EE. Vui lòng yêu cầu người gửi gửi lại ảnh.",
            )
        })?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| {
            media_metadata_unavailable(
                "Khóa giải mã của ảnh WhatsApp không hợp lệ. Vui lòng yêu cầu gửi lại ảnh.",
            )
        })?;
    if decoded.len() != WHATSAPP_MEDIA_SECRET_BYTES {
        return Err(media_metadata_unavailable(
            "Khóa giải mã của ảnh WhatsApp không đầy đủ. Vui lòng yêu cầu gửi lại ảnh.",
        ));
    }
    Ok(decoded)
}

fn decode_optional_whatsapp_hash(
    encoded: Option<&str>,
    field_name: &str,
) -> Result<Option<Vec<u8>>, ConversationCommandError> {
    use base64::Engine as _;

    let Some(encoded) = encoded.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| {
            media_metadata_unavailable(format!(
                "Thông tin kiểm tra {field_name} của ảnh WhatsApp không hợp lệ. Vui lòng yêu cầu gửi lại ảnh."
            ))
        })?;
    if decoded.len() != WHATSAPP_MEDIA_SECRET_BYTES {
        return Err(media_metadata_unavailable(format!(
            "Thông tin kiểm tra {field_name} của ảnh WhatsApp không đầy đủ. Vui lòng yêu cầu gửi lại ảnh."
        )));
    }
    Ok(Some(decoded))
}

fn persist_attachment_thumbnail(
    db_path: &str,
    attachment_id: i64,
    thumbnail_url: &str,
) -> Result<(), ConversationCommandError> {
    let conn = crate::db::open_db_connection(db_path).map_err(|error| {
        ConversationError::new(
            ConversationErrorCode::ProviderFailure,
            format!("Không thể mở DB để lưu ảnh xem trước: {error}"),
        )
        .command_error()
    })?;
    conn.execute(
        "UPDATE message_attachments
         SET thumbnail_url = ?1,
             metadata_json = json_set(
                 CASE WHEN json_valid(metadata_json) THEN metadata_json ELSE '{}' END,
                 '$.preview_resolved', json('true')
             )
         WHERE id = ?2",
        rusqlite::params![thumbnail_url, attachment_id],
    )
    .map_err(|error| {
        ConversationError::new(
            ConversationErrorCode::ProviderFailure,
            format!("Không thể lưu ảnh xem trước: {error}"),
        )
        .command_error()
    })?;
    Ok(())
}

fn persist_attachment_metadata(
    db_path: &str,
    attachment_id: i64,
    metadata_json: &str,
) -> Result<(), ConversationCommandError> {
    let conn = crate::db::open_db_connection(db_path).map_err(|error| {
        ConversationError::new(
            ConversationErrorCode::ProviderFailure,
            format!("Không thể mở DB để lưu metadata ảnh: {error}"),
        )
        .command_error()
    })?;
    conn.execute(
        "UPDATE message_attachments SET metadata_json = ?1 WHERE id = ?2",
        rusqlite::params![metadata_json, attachment_id],
    )
    .map_err(|error| {
        ConversationError::new(
            ConversationErrorCode::ProviderFailure,
            format!("Không thể lưu metadata ảnh: {error}"),
        )
        .command_error()
    })?;
    Ok(())
}

fn whatsapp_media_reference_expired(error: &anyhow::Error) -> bool {
    use whatsapp_rust::ErrorChainExt as _;

    if matches!(error.http_status(), Some(401 | 403 | 404 | 410)) {
        return true;
    }
    let message = error.to_string().to_ascii_lowercase();
    message.contains("reference rejected")
        || message.contains("not found/expired")
        || message.contains("media url expired")
        || message.contains("status: 404")
        || message.contains("status 404")
        || message.contains("status: 410")
        || message.contains("status 410")
}

fn whatsapp_participant_jid(external_id: Option<&str>) -> Option<String> {
    let external_id = external_id
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if external_id.contains('@') {
        return Some(external_id.to_string());
    }
    let phone = super::whatsapp::phone::normalize_e164(external_id, Some("84"))?;
    Some(super::whatsapp::phone::phone_to_direct_jid(&phone))
}

#[tauri::command]
pub async fn download_conversation_attachment(
    state: tauri::State<'_, ConversationState>,
    attachment_id: i64,
) -> Result<crate::conversation::types::ConversationAttachment, ConversationCommandError> {
    let (
        att_id,
        media_type,
        mime_type,
        file_name,
        size_bytes,
        local_path,
        remote_url,
        thumbnail_url,
        metadata_json,
        channel,
        channel_account_id,
        conversation_id,
        raw_payload_json,
        provider_message_id,
        external_thread_id,
        message_direction,
        sender_external_id,
    ) = {
        let conn =
            crate::db::open_db_connection(state.service.repository.db_path()).map_err(|e| {
                ConversationError::new(
                    ConversationErrorCode::InvalidRequest,
                    format!("Failed to open DB: {}", e),
                )
                .command_error()
            })?;

        let mut stmt = conn
            .prepare(
                "SELECT ma.id, ma.attachment_type, ma.mime_type, ma.file_name, ma.size_bytes,
                        ma.local_path, ma.remote_url, ma.thumbnail_url, ma.metadata_json,
                        ca.channel, ca.id, cc.id, m.raw_payload_json,
                        m.provider_message_id, cc.external_thread_id, m.direction,
                        sender.external_id
                 FROM message_attachments ma
                 JOIN messages m ON m.id = ma.message_id
                 JOIN channel_conversations cc ON cc.id = m.conversation_id
                 JOIN channel_accounts ca ON ca.id = cc.channel_account_id
                 LEFT JOIN channel_identities sender ON sender.id = m.sender_identity_id
                 WHERE ma.id = ?1",
            )
            .map_err(|e| {
                ConversationError::new(
                    ConversationErrorCode::InvalidRequest,
                    format!("Failed to prepare statement: {}", e),
                )
                .command_error()
            })?;

        stmt.query_row([attachment_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, i64>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<String>>(13)?,
                row.get::<_, String>(14)?,
                row.get::<_, String>(15)?,
                row.get::<_, Option<String>>(16)?,
            ))
        })
        .map_err(|e| {
            ConversationError::new(
                ConversationErrorCode::InvalidRequest,
                format!("Attachment {} not found: {}", attachment_id, e),
            )
            .command_error()
        })?
    };

    let normalized_size_bytes = match size_bytes {
        Some(value) => Some(u64::try_from(value).map_err(|_| {
            ConversationError::new(
                ConversationErrorCode::InvalidRequest,
                "Kích thước attachment trong DB không hợp lệ",
            )
            .command_error()
        })?),
        None => None,
    };
    if let Some(size_bytes) = normalized_size_bytes {
        ensure_attachment_preview_size(size_bytes)?;
    }
    let attachment_channel = Channel::from_str(&channel).map_err(|error| {
        ConversationError::new(ConversationErrorCode::InvalidRequest, error).command_error()
    })?;

    // A full preview generated by this resolver is reusable. Provider/staging thumbnails are
    // intentionally not marked, because WhatsApp Web shows them immediately and upgrades them
    // to the full media payload in the background.
    if let Some(ref thumb) = thumbnail_url {
        if browser_renderable_thumbnail(thumb)
            && (attachment_preview_is_resolved(&metadata_json)
                || normalized_size_bytes
                    .is_some_and(|size| base64_data_url_decoded_size(thumb) == Some(size)))
        {
            let (width, height) = parse_attachment_dimensions(&metadata_json);
            return Ok(crate::conversation::types::ConversationAttachment {
                id: att_id,
                media_type: media_type.clone(),
                mime_type: crate::conversation::types::ConversationAttachment::resolve_mime_type(
                    &media_type,
                    mime_type.as_deref(),
                ),
                file_name,
                size_bytes: normalized_size_bytes,
                local_path,
                remote_url,
                thumbnail_url: Some(thumb.clone()),
                width,
                height,
                duration_seconds: None,
            });
        }
    }

    // If local_path exists on disk (e.g. outbound attachment, pasted image, local cache),
    // read it through a bounded reader and generate a reusable preview.
    if let Some(ref path_str) = local_path {
        let p = std::path::Path::new(path_str);
        let local_file_exists = tokio::fs::try_exists(p).await.map_err(|error| {
            ConversationError::new(
                ConversationErrorCode::ProviderFailure,
                format!("Không thể kiểm tra file ảnh đã lưu: {error}"),
            )
            .command_error()
        })?;
        if local_file_exists {
            let bytes = read_attachment_file_bounded(p).await?;
            if bytes.is_empty() {
                return Err(ConversationError::new(
                    ConversationErrorCode::UnsupportedMedia,
                    "File ảnh đã lưu đang rỗng",
                )
                .command_error());
            }

            use base64::Engine as _;
            let mime_from_magic = infer::get(&bytes).map(|t| t.mime_type().to_string());
            let resolved_mime = mime_from_magic.unwrap_or_else(|| {
                crate::conversation::types::ConversationAttachment::resolve_mime_type(
                    &media_type,
                    mime_type.as_deref(),
                )
            });
            let b64_data = base64::engine::general_purpose::STANDARD.encode(&bytes);
            let data_url = format!("data:{};base64,{}", resolved_mime, b64_data);
            persist_attachment_thumbnail(state.service.repository.db_path(), att_id, &data_url)?;

            state.service.events.publish(
                crate::conversation::events::ConversationInvalidatedEvent {
                    conversation_id,
                    lead_id: None,
                    channel: attachment_channel,
                    reason: crate::conversation::events::ConversationInvalidationReason::Message,
                    message_id: None,
                },
            );

            let (width, height) = parse_attachment_dimensions(&metadata_json);
            return Ok(crate::conversation::types::ConversationAttachment {
                id: att_id,
                media_type: media_type.clone(),
                mime_type: resolved_mime,
                file_name,
                size_bytes: Some(bytes.len() as u64),
                local_path: local_path.clone(),
                remote_url,
                thumbnail_url: Some(data_url),
                width,
                height,
                duration_seconds: None,
            });
        }
    }

    // Keep a low-resolution thumbnail usable when no full-media upgrade is possible. When the
    // encrypted reference and key are available, fall through to the WhatsApp resolver below.
    if let Some(ref thumb) = thumbnail_url {
        let can_upgrade_whatsapp_preview = attachment_channel == Channel::WhatsApp
            && (whatsapp_reference_is_encrypted(remote_url.as_deref())
                || whatsapp_reference_is_encrypted(Some(thumb)))
            && whatsapp_media_key_available(&metadata_json, raw_payload_json.as_deref());
        if browser_renderable_thumbnail(thumb) && !can_upgrade_whatsapp_preview {
            let (width, height) = parse_attachment_dimensions(&metadata_json);
            return Ok(crate::conversation::types::ConversationAttachment {
                id: att_id,
                media_type: media_type.clone(),
                mime_type: crate::conversation::types::ConversationAttachment::resolve_mime_type(
                    &media_type,
                    mime_type.as_deref(),
                ),
                file_name,
                size_bytes: normalized_size_bytes,
                local_path,
                remote_url,
                thumbnail_url: Some(thumb.clone()),
                width,
                height,
                duration_seconds: None,
            });
        }
    }

    if attachment_channel == Channel::WhatsApp {
        use base64::Engine as _;

        let mut metadata = serde_json::from_str::<serde_json::Value>(&metadata_json)
            .unwrap_or_else(|_| serde_json::json!({}));
        if !metadata.is_object() {
            metadata = serde_json::json!({});
        }
        let mut raw_sender_jid: Option<String> = None;

        if let (Some(metadata_object), Some(raw_payload)) =
            (metadata.as_object_mut(), raw_payload_json.as_deref())
        {
            if let Ok(raw_value) = serde_json::from_str::<serde_json::Value>(raw_payload) {
                raw_sender_jid = raw_value
                    .get("sender_jid")
                    .or_else(|| raw_value.get("senderJid"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string);
                if let Some(raw_attachment) = raw_value
                    .get("attachments")
                    .and_then(serde_json::Value::as_array)
                    .and_then(|attachments| {
                        attachments
                            .iter()
                            .find(|attachment| {
                                attachment
                                    .get("media_type")
                                    .and_then(serde_json::Value::as_str)
                                    .map(|value| match value.to_ascii_lowercase().as_str() {
                                        "image" | "photo" => "image",
                                        "video" => "video",
                                        "audio" | "voice" | "ptt" => "audio",
                                        "sticker" => "sticker",
                                        _ => "file",
                                    })
                                    == Some(media_type.as_str())
                            })
                            .or_else(|| attachments.first())
                    })
                {
                    for key in [
                        "direct_path",
                        "media_key",
                        "file_sha256",
                        "file_enc_sha256",
                        "width",
                        "height",
                    ] {
                        let current_missing = metadata_object
                            .get(key)
                            .map(|value| {
                                value.is_null()
                                    || value
                                        .as_str()
                                        .map(|text| text.trim().is_empty())
                                        .unwrap_or(false)
                            })
                            .unwrap_or(true);
                        let raw_value = raw_attachment.get(key).filter(|value| {
                            !value.is_null()
                                && value
                                    .as_str()
                                    .map(|text| !text.trim().is_empty())
                                    .unwrap_or(true)
                        });
                        if current_missing {
                            if let Some(raw_value) = raw_value {
                                metadata_object.insert(key.to_string(), raw_value.clone());
                            }
                        }
                    }
                }
            }
        }

        let direct_path = metadata
            .get("direct_path")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| {
                remote_url.as_ref().and_then(|remote_url| {
                    if remote_url.starts_with('/') {
                        Some(remote_url.clone())
                    } else {
                        url::Url::parse(remote_url)
                            .ok()
                            .map(|url| url.path().to_string())
                            .filter(|path| !path.is_empty() && path != "/")
                    }
                })
            })
            .ok_or_else(|| {
                media_metadata_unavailable(
                    "Ảnh lịch sử thiếu đường dẫn tải WhatsApp. Vui lòng yêu cầu người gửi gửi lại ảnh.",
                )
            })?;

        if let Some(metadata_object) = metadata.as_object_mut() {
            let needs_direct_path = metadata_object
                .get("direct_path")
                .and_then(serde_json::Value::as_str)
                .map(|value| value.trim().is_empty())
                .unwrap_or(true);
            if needs_direct_path {
                metadata_object.insert(
                    "direct_path".to_string(),
                    serde_json::Value::String(direct_path.clone()),
                );
            }
        }

        let media_key_bytes = decode_required_whatsapp_media_key(
            metadata
                .get("media_key")
                .and_then(serde_json::Value::as_str),
        )?;
        let file_sha256_bytes = decode_optional_whatsapp_hash(
            metadata
                .get("file_sha256")
                .and_then(serde_json::Value::as_str),
            "file_sha256",
        )?;
        let file_enc_sha256_bytes = decode_optional_whatsapp_hash(
            metadata
                .get("file_enc_sha256")
                .and_then(serde_json::Value::as_str),
            "file_enc_sha256",
        )?;

        let mut effective_metadata_json = metadata.to_string();
        if effective_metadata_json != metadata_json {
            persist_attachment_metadata(
                state.service.repository.db_path(),
                att_id,
                &effective_metadata_json,
            )?;
        }

        let wa_media_type = match media_type.to_lowercase().as_str() {
            "image" | "photo" => whatsapp_rust::wacore::download::MediaType::Image,
            "video" => whatsapp_rust::wacore::download::MediaType::Video,
            "audio" | "voice" | "ptt" => whatsapp_rust::wacore::download::MediaType::Audio,
            "sticker" => whatsapp_rust::wacore::download::MediaType::Sticker,
            _ => whatsapp_rust::wacore::download::MediaType::Document,
        };

        let mut params = whatsapp_rust::download::DownloadParams {
            direct_path,
            media_key: Some(media_key_bytes.clone()),
            file_sha256: file_sha256_bytes.unwrap_or_default(),
            file_enc_sha256: file_enc_sha256_bytes,
            file_length: normalized_size_bytes.unwrap_or(0),
            media_type: wa_media_type,
        };

        let client = state
            .whatsapp_supervisor
            .get_client(channel_account_id)
            .await
            .ok_or_else(|| {
                ConversationError::new(
                    ConversationErrorCode::ProviderFailure,
                    format!("WhatsApp account {} is not connected", channel_account_id),
                )
                .command_error()
            })?;

        let initial_download = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            client.download_media(&params),
        )
        .await
        .map_err(|_| {
            ConversationError::new(
                ConversationErrorCode::ProviderFailure,
                "Quá thời gian tải media từ máy chủ WhatsApp (15s)",
            )
            .command_error()
        })?;

        let bytes = match initial_download {
            Ok(bytes) => bytes,
            Err(error) if whatsapp_media_reference_expired(&error) => {
                let provider_message_id = provider_message_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        media_metadata_unavailable(
                            "Ảnh lịch sử thiếu mã tin nhắn để làm mới URL. Vui lòng yêu cầu người gửi gửi lại ảnh.",
                        )
                    })?;
                let participant_jid =
                    if external_thread_id.ends_with("@g.us") && message_direction == "inbound" {
                        raw_sender_jid
                            .as_deref()
                            .and_then(|value| whatsapp_participant_jid(Some(value)))
                            .or_else(|| whatsapp_participant_jid(sender_external_id.as_deref()))
                    } else {
                        None
                    };
                let reupload_request =
                    crate::conversation::whatsapp::client::WhatsAppMediaReuploadRequest {
                        provider_message_id: provider_message_id.to_string(),
                        chat_jid: external_thread_id.clone(),
                        media_key: media_key_bytes.clone(),
                        is_from_me: message_direction == "outbound",
                        participant_jid,
                    };
                let reupload_result = tokio::time::timeout(
                    std::time::Duration::from_secs(35),
                    client.request_media_reupload(&reupload_request),
                )
                .await
                .map_err(|_| {
                    ConversationError::new(
                        ConversationErrorCode::ProviderFailure,
                        "Quá thời gian yêu cầu WhatsApp làm mới URL media",
                    )
                    .command_error()
                })?
                .map_err(|error| {
                    ConversationError::new(
                        ConversationErrorCode::ProviderFailure,
                        format!("Không thể yêu cầu WhatsApp làm mới URL media: {error}"),
                    )
                    .command_error()
                })?;

                let refreshed_direct_path = match reupload_result {
                    crate::conversation::whatsapp::client::WhatsAppMediaReuploadResult::Success {
                        direct_path,
                    } if !direct_path.trim().is_empty() => direct_path,
                    crate::conversation::whatsapp::client::WhatsAppMediaReuploadResult::Success {
                        ..
                    }
                    | crate::conversation::whatsapp::client::WhatsAppMediaReuploadResult::NotFound => {
                        return Err(media_metadata_unavailable(
                            "WhatsApp không còn lưu ảnh lịch sử này. Vui lòng yêu cầu người gửi gửi lại ảnh.",
                        ));
                    }
                    crate::conversation::whatsapp::client::WhatsAppMediaReuploadResult::DecryptionError => {
                        return Err(media_metadata_unavailable(
                            "WhatsApp không thể xác thực khóa của ảnh lịch sử. Vui lòng yêu cầu gửi lại ảnh.",
                        ));
                    }
                    crate::conversation::whatsapp::client::WhatsAppMediaReuploadResult::GeneralError => {
                        return Err(ConversationError::new(
                            ConversationErrorCode::ProviderFailure,
                            "WhatsApp tạm thời không thể làm mới URL media",
                        )
                        .command_error());
                    }
                };

                params.direct_path = refreshed_direct_path.clone();
                if let Some(metadata_object) = metadata.as_object_mut() {
                    metadata_object.insert(
                        "direct_path".to_string(),
                        serde_json::Value::String(refreshed_direct_path),
                    );
                }
                effective_metadata_json = metadata.to_string();
                persist_attachment_metadata(
                    state.service.repository.db_path(),
                    att_id,
                    &effective_metadata_json,
                )?;

                tokio::time::timeout(
                    std::time::Duration::from_secs(15),
                    client.download_media(&params),
                )
                .await
                .map_err(|_| {
                    ConversationError::new(
                        ConversationErrorCode::ProviderFailure,
                        "Quá thời gian tải lại media sau khi WhatsApp làm mới URL",
                    )
                    .command_error()
                })?
                .map_err(|error| {
                    ConversationError::new(
                        ConversationErrorCode::ProviderFailure,
                        format!("Không thể tải media sau khi làm mới URL: {error}"),
                    )
                    .command_error()
                })?
            }
            Err(error) => {
                return Err(ConversationError::new(
                    ConversationErrorCode::ProviderFailure,
                    format!("Không thể tải và giải mã media WhatsApp: {error}"),
                )
                .command_error());
            }
        };

        if bytes.is_empty() {
            return Err(ConversationError::new(
                ConversationErrorCode::ProviderFailure,
                "Máy chủ WhatsApp trả về file ảnh rỗng",
            )
            .command_error());
        }
        ensure_attachment_preview_size(bytes.len() as u64)?;

        let mime_from_magic = infer::get(&bytes).map(|t| t.mime_type().to_string());
        let resolved_mime = mime_from_magic.unwrap_or_else(|| {
            crate::conversation::types::ConversationAttachment::resolve_mime_type(
                &media_type,
                mime_type.as_deref(),
            )
        });
        let b64_data = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let data_url = format!("data:{};base64,{}", resolved_mime, b64_data);
        persist_attachment_thumbnail(state.service.repository.db_path(), att_id, &data_url)?;

        state
            .service
            .events
            .publish(crate::conversation::events::ConversationInvalidatedEvent {
                conversation_id,
                lead_id: None,
                channel: Channel::WhatsApp,
                reason: crate::conversation::events::ConversationInvalidationReason::Message,
                message_id: None,
            });

        let (width, height) = parse_attachment_dimensions(&effective_metadata_json);
        return Ok(crate::conversation::types::ConversationAttachment {
            id: att_id,
            media_type: media_type.clone(),
            mime_type: resolved_mime,
            file_name,
            size_bytes: Some(bytes.len() as u64),
            local_path,
            remote_url,
            thumbnail_url: Some(data_url),
            width,
            height,
            duration_seconds: None,
        });
    }

    let (width, height) = parse_attachment_dimensions(&metadata_json);
    Ok(crate::conversation::types::ConversationAttachment {
        id: att_id,
        media_type: media_type.clone(),
        mime_type: crate::conversation::types::ConversationAttachment::resolve_mime_type(
            &media_type,
            mime_type.as_deref(),
        ),
        file_name,
        size_bytes: normalized_size_bytes,
        local_path,
        remote_url,
        thumbnail_url,
        width,
        height,
        duration_seconds: None,
    })
}

fn parse_attachment_dimensions(metadata_json: &str) -> (Option<u32>, Option<u32>) {
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(metadata_json) {
        let w = val.get("width").and_then(|v| v.as_u64()).map(|v| v as u32);
        let h = val.get("height").and_then(|v| v.as_u64()).map(|v| v as u32);
        (w, h)
    } else {
        (None, None)
    }
}
