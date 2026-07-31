use super::outreach::*;
use zca_rs::models::{SendMessageResponse, SendMessageResult};

#[test]
fn provider_receipt_prefers_text_message_id() {
    let receipt = ZaloProviderReceipt::from_response(SendMessageResponse {
        message: Some(SendMessageResult { msg_id: 42 }),
        attachment: vec![SendMessageResult { msg_id: 99 }],
    });
    assert_eq!(receipt.primary_message_id.as_deref(), Some("42"));
    assert_eq!(receipt.attachment_message_ids, vec!["99"]);
}

#[test]
fn provider_receipt_uses_first_attachment_when_text_is_absent() {
    let receipt = ZaloProviderReceipt::from_response(SendMessageResponse {
        message: None,
        attachment: vec![SendMessageResult { msg_id: 99 }],
    });
    assert_eq!(receipt.primary_message_id.as_deref(), Some("99"));
    assert_eq!(receipt.attachment_message_ids, vec!["99"]);
}

#[test]
fn provider_receipt_empty_response_returns_none_primary_id() {
    let receipt = ZaloProviderReceipt::from_response(SendMessageResponse {
        message: None,
        attachment: vec![],
    });
    assert_eq!(receipt.primary_message_id, None);
    assert!(receipt.attachment_message_ids.is_empty());
}
