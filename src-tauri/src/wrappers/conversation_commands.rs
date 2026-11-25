#![allow(
    unused_imports,
    unused_variables,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
use crate::CoreHandle;
#[allow(unused_imports)]
use rustsale_core::conversation::commands::*;
use rustsale_core::conversation::error::{
    ConversationCommandError, ConversationError, ConversationErrorCode,
};
use rustsale_core::conversation::facebook::supervisor::FacebookMessengerSupervisor;
use rustsale_core::conversation::service::ConversationService;
use rustsale_core::conversation::types::{
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
use rustsale_core::conversation::whatsapp::login::WhatsAppLoginManager;
use rustsale_core::conversation::whatsapp::supervisor::WhatsAppSupervisor;
use rustsale_core::conversation::zalo::supervisor::ZaloSessionSupervisor;
use rustsale_core::host::{AppHandle, State};
use std::sync::Arc;

#[tauri::command]
pub fn list_conversation_inbox(
    core: tauri::State<'_, CoreHandle>,
) -> Result<Vec<InboxItem>, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::list_conversation_inbox(st_state)
}

#[tauri::command]
pub fn mark_conversation_read(
    core: tauri::State<'_, CoreHandle>,
    key: String,
) -> Result<u64, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::mark_conversation_read(st_state, key)
}

#[tauri::command]
pub fn get_contact_timeline(
    core: tauri::State<'_, CoreHandle>,
    lead_id: i64,
    active_channels: Vec<Channel>,
    before: Option<TimelineCursor>,
    limit: u32,
) -> Result<TimelinePage, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::get_contact_timeline(
        st_state,
        lead_id,
        active_channels,
        before,
        limit,
    )
}

#[tauri::command]
pub fn get_conversation_timeline(
    core: tauri::State<'_, CoreHandle>,
    conversation_id: i64,
    before: Option<TimelineCursor>,
    limit: u32,
) -> Result<TimelinePage, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::get_conversation_timeline(
        st_state,
        conversation_id,
        before,
        limit,
    )
}

#[tauri::command]
pub fn get_identity_timeline(
    core: tauri::State<'_, CoreHandle>,
    identity_id: i64,
    before: Option<TimelineCursor>,
    limit: u32,
) -> Result<TimelinePage, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::get_identity_timeline(
        st_state,
        identity_id,
        before,
        limit,
    )
}

#[tauri::command]
pub fn link_conversation_identity(
    core: tauri::State<'_, CoreHandle>,
    request: LinkIdentityRequest,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::link_conversation_identity(st_state, request)
}

#[tauri::command]
pub fn unlink_conversation_identity(
    core: tauri::State<'_, CoreHandle>,
    request: UnlinkIdentityRequest,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::unlink_conversation_identity(st_state, request)
}

#[tauri::command]
pub fn unlink_conversation_lead(
    core: tauri::State<'_, CoreHandle>,
    request: UnlinkLeadRequest,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::unlink_conversation_lead(st_state, request)
}

#[tauri::command]
pub fn relink_conversation_lead(
    core: tauri::State<'_, CoreHandle>,
    request: RelinkLeadRequest,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::relink_conversation_lead(st_state, request)
}

#[tauri::command]
pub fn send_conversation_message(
    core: tauri::State<'_, CoreHandle>,
    request: SendConversationMessageRequest,
) -> Result<ConversationMessage, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::send_conversation_message(st_state, request)
}

#[tauri::command]
pub async fn get_conversation_runtime_status(
    core: tauri::State<'_, CoreHandle>,
) -> Result<ConversationRuntimeStatus, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::get_conversation_runtime_status(st_state).await
}

#[tauri::command]
pub async fn restart_conversation_account(
    core: tauri::State<'_, CoreHandle>,
    provider_account_key: String,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::restart_conversation_account(
        st_state,
        provider_account_key,
    )
    .await
}

#[tauri::command]
pub fn delete_conversation(
    core: tauri::State<'_, CoreHandle>,
    request: DeleteConversationRequest,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::delete_conversation(st_state, request)
}

#[tauri::command]
pub fn delete_conversations_batch(
    core: tauri::State<'_, CoreHandle>,
    requests: Vec<DeleteConversationRequest>,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::delete_conversations_batch(st_state, requests)
}

#[tauri::command]
pub fn start_direct_conversation(
    core: tauri::State<'_, CoreHandle>,
    request: StartDirectConversationRequest,
) -> Result<ConversationMessage, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::start_direct_conversation(st_state, request)
}

#[tauri::command]
pub fn send_conversation_batch(
    core: tauri::State<'_, CoreHandle>,
    request: SendConversationBatchRequest,
) -> Result<SendConversationBatchResult, ConversationCommandError> {
    let app = core.handle();
    let st_state = app.state::<ConversationState>().map_err(|e| {
        rustsale_core::conversation::error::ConversationError::new(
            rustsale_core::conversation::error::ConversationErrorCode::ProviderFailure,
            e.to_string(),
        )
        .command_error()
    })?;
    rustsale_core::conversation::commands::send_conversation_batch(st_state, request)
}

#[tauri::command]
pub async fn stage_conversation_attachment_paths(
    core: tauri::State<'_, CoreHandle>,
    conversation_id: i64,
    paths: Vec<String>,
) -> Result<Vec<StagedAttachmentResponse>, ConversationCommandError> {
    let app = core.handle();
    let st_state = app.state::<ConversationState>().map_err(|e| {
        rustsale_core::conversation::error::ConversationError::new(
            rustsale_core::conversation::error::ConversationErrorCode::ProviderFailure,
            e.to_string(),
        )
        .command_error()
    })?;
    rustsale_core::conversation::commands::stage_conversation_attachment_paths(
        st_state,
        conversation_id,
        paths,
    )
    .await
}

#[tauri::command]
pub async fn stage_conversation_clipboard_image(
    core: tauri::State<'_, CoreHandle>,
    request: StageConversationClipboardImageRequest,
) -> Result<StagedAttachmentResponse, ConversationCommandError> {
    let app = core.handle();
    let st_state = app.state::<ConversationState>().map_err(|e| {
        rustsale_core::conversation::error::ConversationError::new(
            rustsale_core::conversation::error::ConversationErrorCode::ProviderFailure,
            e.to_string(),
        )
        .command_error()
    })?;
    rustsale_core::conversation::commands::stage_conversation_clipboard_image(st_state, request)
        .await
}

#[tauri::command]
pub async fn discard_conversation_staged_attachment(
    core: tauri::State<'_, CoreHandle>,
    staged_attachment_id: String,
) -> Result<(), ConversationCommandError> {
    let app = core.handle();
    let st_state = app.state::<ConversationState>().map_err(|e| {
        rustsale_core::conversation::error::ConversationError::new(
            rustsale_core::conversation::error::ConversationErrorCode::ProviderFailure,
            e.to_string(),
        )
        .command_error()
    })?;
    rustsale_core::conversation::commands::discard_conversation_staged_attachment(
        st_state,
        staged_attachment_id,
    )
    .await
}

#[tauri::command]
pub async fn search_zalo_stickers(
    core: tauri::State<'_, CoreHandle>,
    conversation_id: i64,
    query: String,
    limit: u32,
) -> Result<
    Vec<rustsale_core::conversation::zalo::client::ZaloStickerReference>,
    ConversationCommandError,
> {
    let app = core.handle();
    let st_state = app.state::<ConversationState>().map_err(|e| {
        rustsale_core::conversation::error::ConversationError::new(
            rustsale_core::conversation::error::ConversationErrorCode::ProviderFailure,
            e.to_string(),
        )
        .command_error()
    })?;
    rustsale_core::conversation::commands::search_zalo_stickers(
        st_state,
        conversation_id,
        query,
        limit,
    )
    .await
}

#[tauri::command]
pub async fn get_zalo_sticker_detail(
    core: tauri::State<'_, CoreHandle>,
    conversation_id: i64,
    sticker_id: u32,
) -> Result<
    Option<rustsale_core::conversation::zalo::client::ZaloStickerReference>,
    ConversationCommandError,
> {
    let app = core.handle();
    let st_state = app.state::<ConversationState>().map_err(|e| {
        rustsale_core::conversation::error::ConversationError::new(
            rustsale_core::conversation::error::ConversationErrorCode::ProviderFailure,
            e.to_string(),
        )
        .command_error()
    })?;
    rustsale_core::conversation::commands::get_zalo_sticker_detail(
        st_state,
        conversation_id,
        sticker_id,
    )
    .await
}

#[tauri::command]
pub async fn recall_conversation_message(
    core: tauri::State<'_, CoreHandle>,
    request: rustsale_core::conversation::types::RecallMessageRequest,
) -> Result<ConversationMessage, ConversationCommandError> {
    let app = core.handle();
    let st_state = app.state::<ConversationState>().map_err(|e| {
        rustsale_core::conversation::error::ConversationError::new(
            rustsale_core::conversation::error::ConversationErrorCode::ProviderFailure,
            e.to_string(),
        )
        .command_error()
    })?;
    rustsale_core::conversation::commands::recall_conversation_message(st_state, request).await
}

#[tauri::command]
pub fn list_conversation_quick_templates(
    core: tauri::State<'_, CoreHandle>,
    request: ListConversationQuickTemplatesRequest,
) -> Result<Vec<ConversationQuickTemplate>, ConversationCommandError> {
    let app = core.handle();
    let st_state = app.state::<ConversationState>().map_err(|e| {
        rustsale_core::conversation::error::ConversationError::new(
            rustsale_core::conversation::error::ConversationErrorCode::ProviderFailure,
            e.to_string(),
        )
        .command_error()
    })?;
    rustsale_core::conversation::commands::list_conversation_quick_templates(st_state, request)
}

#[tauri::command]
pub fn create_conversation_quick_template(
    core: tauri::State<'_, CoreHandle>,
    request: CreateConversationQuickTemplateRequest,
) -> Result<ConversationQuickTemplate, ConversationCommandError> {
    let app = core.handle();
    let st_state = app.state::<ConversationState>().map_err(|e| {
        rustsale_core::conversation::error::ConversationError::new(
            rustsale_core::conversation::error::ConversationErrorCode::ProviderFailure,
            e.to_string(),
        )
        .command_error()
    })?;
    rustsale_core::conversation::commands::create_conversation_quick_template(st_state, request)
}

#[tauri::command]
pub fn update_conversation_quick_template(
    core: tauri::State<'_, CoreHandle>,
    request: UpdateConversationQuickTemplateRequest,
) -> Result<ConversationQuickTemplate, ConversationCommandError> {
    let app = core.handle();
    let st_state = app.state::<ConversationState>().map_err(|e| {
        rustsale_core::conversation::error::ConversationError::new(
            rustsale_core::conversation::error::ConversationErrorCode::ProviderFailure,
            e.to_string(),
        )
        .command_error()
    })?;
    rustsale_core::conversation::commands::update_conversation_quick_template(st_state, request)
}

#[tauri::command]
pub fn delete_conversation_quick_template(
    core: tauri::State<'_, CoreHandle>,
    request: DeleteConversationQuickTemplateRequest,
) -> Result<(), ConversationCommandError> {
    let app = core.handle();
    let st_state = app.state::<ConversationState>().map_err(|e| {
        rustsale_core::conversation::error::ConversationError::new(
            rustsale_core::conversation::error::ConversationErrorCode::ProviderFailure,
            e.to_string(),
        )
        .command_error()
    })?;
    rustsale_core::conversation::commands::delete_conversation_quick_template(st_state, request)
}

#[tauri::command]
pub fn mark_conversation_quick_template_used(
    core: tauri::State<'_, CoreHandle>,
    request: MarkConversationQuickTemplateUsedRequest,
) -> Result<(), ConversationCommandError> {
    let app = core.handle();
    let st_state = app.state::<ConversationState>().map_err(|e| {
        rustsale_core::conversation::error::ConversationError::new(
            rustsale_core::conversation::error::ConversationErrorCode::ProviderFailure,
            e.to_string(),
        )
        .command_error()
    })?;
    rustsale_core::conversation::commands::mark_conversation_quick_template_used(st_state, request)
}

#[tauri::command]
pub async fn set_conversation_reaction(
    core: tauri::State<'_, CoreHandle>,
    request: SetConversationReactionRequest,
) -> Result<ConversationReactionMutationResult, ConversationCommandError> {
    let app = core.handle();
    let st_state = app.state::<ConversationState>().map_err(|e| {
        rustsale_core::conversation::error::ConversationError::new(
            rustsale_core::conversation::error::ConversationErrorCode::ProviderFailure,
            e.to_string(),
        )
        .command_error()
    })?;
    rustsale_core::conversation::commands::set_conversation_reaction(st_state, request).await
}

#[tauri::command]
pub fn forward_conversation_message(
    core: tauri::State<'_, CoreHandle>,
    request: ForwardConversationMessageRequest,
) -> Result<ForwardConversationMessageResult, ConversationCommandError> {
    let app = core.handle();
    let st_state = app.state::<ConversationState>().map_err(|e| {
        rustsale_core::conversation::error::ConversationError::new(
            rustsale_core::conversation::error::ConversationErrorCode::ProviderFailure,
            e.to_string(),
        )
        .command_error()
    })?;
    rustsale_core::conversation::commands::forward_conversation_message(st_state, request)
}

#[tauri::command]
pub fn get_conversation_forward_operation(
    core: tauri::State<'_, CoreHandle>,
    client_operation_id: String,
) -> Result<ForwardConversationMessageResult, ConversationCommandError> {
    let app = core.handle();
    let st_state = app.state::<ConversationState>().map_err(|e| {
        rustsale_core::conversation::error::ConversationError::new(
            rustsale_core::conversation::error::ConversationErrorCode::ProviderFailure,
            e.to_string(),
        )
        .command_error()
    })?;
    rustsale_core::conversation::commands::get_conversation_forward_operation(
        st_state,
        client_operation_id,
    )
}

#[tauri::command]
pub fn retry_conversation_forward_target(
    core: tauri::State<'_, CoreHandle>,
    request: RetryConversationForwardTargetRequest,
) -> Result<ConversationForwardTargetResult, ConversationCommandError> {
    let app = core.handle();
    let st_state = app.state::<ConversationState>().map_err(|e| {
        rustsale_core::conversation::error::ConversationError::new(
            rustsale_core::conversation::error::ConversationErrorCode::ProviderFailure,
            e.to_string(),
        )
        .command_error()
    })?;
    rustsale_core::conversation::commands::retry_conversation_forward_target(st_state, request)
}

#[tauri::command]
pub fn list_conversation_mention_candidates(
    core: tauri::State<'_, CoreHandle>,
    request: ListConversationMentionCandidatesRequest,
) -> Result<Vec<ConversationMentionCandidate>, ConversationCommandError> {
    let app = core.handle();
    let st_state = app.state::<ConversationState>().map_err(|e| {
        rustsale_core::conversation::error::ConversationError::new(
            rustsale_core::conversation::error::ConversationErrorCode::ProviderFailure,
            e.to_string(),
        )
        .command_error()
    })?;
    rustsale_core::conversation::commands::list_conversation_mention_candidates(st_state, request)
}

#[tauri::command]
pub fn pin_conversation_message(
    core: tauri::State<'_, CoreHandle>,
    request: PinMessageRequest,
) -> Result<PinnedMessage, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::pin_conversation_message(st_state, request)
}

#[tauri::command]
pub fn unpin_conversation_message(
    core: tauri::State<'_, CoreHandle>,
    request: UnpinMessageRequest,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::unpin_conversation_message(st_state, request)
}

#[tauri::command]
pub fn list_conversation_pinned_messages(
    core: tauri::State<'_, CoreHandle>,
    conversation_id: i64,
) -> Result<Vec<PinnedMessage>, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::list_conversation_pinned_messages(
        st_state,
        conversation_id,
    )
}

#[tauri::command]
pub async fn sync_facebook_conversations(
    core: tauri::State<'_, CoreHandle>,
    page_id: Option<String>,
) -> Result<usize, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::sync_facebook_conversations(st_state, page_id).await
}

#[tauri::command]
pub fn ingest_facebook_webhook_events(
    core: tauri::State<'_, CoreHandle>,
    page_id: String,
    events: Vec<serde_json::Value>,
) -> Result<usize, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::ingest_facebook_webhook_events(st_state, page_id, events)
}

#[tauri::command]
pub async fn start_whatsapp_qr_login(
    core: tauri::State<'_, CoreHandle>,
    attempt_id: String,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::start_whatsapp_qr_login(st_state, attempt_id).await
}

#[tauri::command]
pub fn cancel_whatsapp_qr_login(
    core: tauri::State<'_, CoreHandle>,
    attempt_id: String,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::cancel_whatsapp_qr_login(st_state, attempt_id)
}

#[tauri::command]
pub fn get_managed_whatsapp_accounts(
    core: tauri::State<'_, CoreHandle>,
) -> Result<Vec<rustsale_core::conversation::whatsapp::supervisor::ConnectedWhatsAppAccount>, String>
{
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::get_managed_whatsapp_accounts(st_state)
}

#[tauri::command]
pub async fn disconnect_whatsapp_account(
    core: tauri::State<'_, CoreHandle>,
    channel_account_id: i64,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::disconnect_whatsapp_account(st_state, channel_account_id)
        .await
}

#[tauri::command]
pub fn create_whatsapp_campaign(
    core: tauri::State<'_, CoreHandle>,
    req: rustsale_core::conversation::whatsapp::campaign::CreateCampaignRequest,
) -> Result<i64, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::create_whatsapp_campaign(st_state, req)
}

#[tauri::command]
pub fn populate_whatsapp_campaign_recipients(
    core: tauri::State<'_, CoreHandle>,
    campaign_id: i64,
    recipients: Vec<rustsale_core::conversation::whatsapp::campaign::CampaignRecipientInput>,
    template: Option<String>,
) -> Result<usize, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::populate_whatsapp_campaign_recipients(
        st_state,
        campaign_id,
        recipients,
        template,
    )
}

#[tauri::command]
pub async fn start_whatsapp_campaign(
    core: tauri::State<'_, CoreHandle>,
    campaign_id: i64,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::start_whatsapp_campaign(st_state, campaign_id).await
}

#[tauri::command]
pub fn pause_whatsapp_campaign(
    core: tauri::State<'_, CoreHandle>,
    campaign_id: i64,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::pause_whatsapp_campaign(st_state, campaign_id)
}

#[tauri::command]
pub fn resume_whatsapp_campaign(
    core: tauri::State<'_, CoreHandle>,
    campaign_id: i64,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::resume_whatsapp_campaign(st_state, campaign_id)
}

#[tauri::command]
pub fn cancel_whatsapp_campaign(
    core: tauri::State<'_, CoreHandle>,
    campaign_id: i64,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::cancel_whatsapp_campaign(st_state, campaign_id)
}

#[tauri::command]
pub fn get_whatsapp_campaign(
    core: tauri::State<'_, CoreHandle>,
    campaign_id: i64,
) -> Result<rustsale_core::conversation::whatsapp::campaign::WhatsAppCampaignRecord, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::get_whatsapp_campaign(st_state, campaign_id)
}

#[tauri::command]
pub fn list_whatsapp_campaigns(
    core: tauri::State<'_, CoreHandle>,
) -> Result<Vec<rustsale_core::conversation::whatsapp::campaign::WhatsAppCampaignRecord>, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::list_whatsapp_campaigns(st_state)
}

#[tauri::command]
pub fn get_whatsapp_campaign_recipients(
    core: tauri::State<'_, CoreHandle>,
    campaign_id: i64,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<rustsale_core::conversation::whatsapp::campaign::CampaignRecipientRecord>, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::get_whatsapp_campaign_recipients(
        st_state,
        campaign_id,
        limit,
        offset,
    )
}

#[tauri::command]
pub async fn sync_zalo_group_chat_history(
    core: tauri::State<'_, CoreHandle>,
    conversation_id: i64,
) -> Result<usize, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::sync_zalo_group_chat_history(st_state, conversation_id)
        .await
}

#[tauri::command]
pub async fn sync_whatsapp_conversations(
    core: tauri::State<'_, CoreHandle>,
) -> Result<usize, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::sync_whatsapp_conversations(st_state).await
}

#[tauri::command]
pub async fn download_conversation_attachment(
    core: tauri::State<'_, CoreHandle>,
    attachment_id: i64,
) -> Result<rustsale_core::conversation::types::ConversationAttachment, ConversationCommandError> {
    let app = core.handle();
    let st_state = app.state::<ConversationState>().map_err(|e| {
        rustsale_core::conversation::error::ConversationError::new(
            rustsale_core::conversation::error::ConversationErrorCode::ProviderFailure,
            e.to_string(),
        )
        .command_error()
    })?;
    rustsale_core::conversation::commands::download_conversation_attachment(st_state, attachment_id)
        .await
}

#[tauri::command]
pub fn resume_conversation_deltas(
    core: tauri::State<'_, CoreHandle>,
    after_sequence: u64,
    limit: Option<usize>,
) -> Result<Vec<rustsale_core::conversation::events::ConversationMessageDeltaEvent>, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::resume_conversation_deltas(
        st_state,
        after_sequence,
        limit,
    )
}

#[tauri::command]
pub fn get_conversation_high_water_mark(
    core: tauri::State<'_, CoreHandle>,
) -> Result<rustsale_core::conversation::types::ConversationHighWaterMark, String> {
    let app = core.handle();
    let st_state = app
        .state::<ConversationState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::conversation::commands::get_conversation_high_water_mark(st_state)
}
