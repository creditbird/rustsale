use crate::conversation::repository::AppliedMessage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZaloProviderReceipt {
    pub primary_message_id: Option<String>,
    pub attachment_message_ids: Vec<String>,
}

impl ZaloProviderReceipt {
    pub fn from_response(response: zca_rs::models::SendMessageResponse) -> Self {
        let text_message_id = response.message.map(|message| message.msg_id.to_string());
        let attachment_message_ids = response
            .attachment
            .into_iter()
            .map(|message| message.msg_id.to_string())
            .collect::<Vec<_>>();
        let primary_message_id =
            text_message_id.or_else(|| attachment_message_ids.first().cloned());
        Self {
            primary_message_id,
            attachment_message_ids,
        }
    }

    pub fn mock(local_request_id: &str) -> Self {
        Self {
            primary_message_id: Some(format!("mock-{local_request_id}")),
            attachment_message_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OutreachConversationRecord {
    pub lead_id: i64,
    pub provider_account_key: String,
    pub account_display_name: Option<String>,
    pub external_user_id: String,
    pub user_display_name: Option<String>,
    pub user_avatar_url: Option<String>,
    pub template_id: i64,
    pub source: String,
    pub text: String,
    pub local_request_id: String,
    pub provider_receipt: ZaloProviderReceipt,
    pub sent_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedOutreachMessage {
    pub applied: AppliedMessage,
    pub lead_id: i64,
}
