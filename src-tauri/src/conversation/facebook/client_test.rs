use super::client::*;
use serde_json::json;

#[test]
fn test_parse_facebook_conversations_page() {
    let raw_json = json!({
        "data": [
            {
                "id": "t_100200300",
                "updated_time": "2026-08-24T02:00:00+0000",
                "participants": {
                    "data": [
                        {
                            "id": "page_999",
                            "name": "RustSale Page",
                            "email": "page_999@facebook.com"
                        },
                        {
                            "id": "psid_12345",
                            "name": "Nguyen Van A",
                            "email": "12345@facebook.com"
                        }
                    ]
                },
                "messages": {
                    "data": [
                        {
                            "id": "m_mid_001",
                            "created_time": "2026-08-24T01:59:00+0000",
                            "from": {
                                "id": "psid_12345",
                                "name": "Nguyen Van A"
                            },
                            "message": "Xin chào shop!",
                            "attachments": {
                                "data": [
                                    {
                                        "id": "att_001",
                                        "mime_type": "image/jpeg",
                                        "name": "photo.jpg",
                                        "file_url": "https://example.com/photo.jpg",
                                        "image_data": {
                                            "url": "https://example.com/photo.jpg",
                                            "preview_url": "https://example.com/thumb.jpg"
                                        }
                                    }
                                ]
                            }
                        },
                        {
                            "id": "m_mid_002",
                            "created_time": "2026-08-24T02:00:00+0000",
                            "from": {
                                "id": "page_999",
                                "name": "RustSale Page"
                            },
                            "message": "Dạ chào bạn, shop có thể hỗ trợ gì ạ?"
                        }
                    ]
                }
            }
        ],
        "paging": {
            "cursors": {
                "before": "curs_before",
                "after": "curs_after"
            }
        }
    });

    let page: FacebookConversationsPage = serde_json::from_value(raw_json).unwrap();
    assert_eq!(page.data.len(), 1);
    let thread = &page.data[0];
    assert_eq!(thread.id, "t_100200300");
    assert_eq!(thread.updated_time, "2026-08-24T02:00:00+0000");

    let participants = thread.participants.as_ref().unwrap();
    assert_eq!(participants.data.len(), 2);
    assert_eq!(participants.data[1].id, "psid_12345");
    assert_eq!(participants.data[1].name.as_deref(), Some("Nguyen Van A"));

    let messages = thread.messages.as_ref().unwrap();
    assert_eq!(messages.data.len(), 2);
    assert_eq!(messages.data[0].id, "m_mid_001");
    assert_eq!(messages.data[0].message.as_deref(), Some("Xin chào shop!"));
    assert_eq!(messages.data[1].id, "m_mid_002");
    assert_eq!(messages.data[1].from.as_ref().unwrap().id, "page_999");
}

#[test]
fn test_parse_send_message_response() {
    let raw_json = json!({
        "recipient_id": "psid_12345",
        "message_id": "m_mid_resp_999"
    });

    let resp: SendFacebookMessageResponse = serde_json::from_value(raw_json).unwrap();
    assert_eq!(resp.recipient_id, "psid_12345");
    assert_eq!(resp.message_id, "m_mid_resp_999");
}

#[test]
fn test_parse_meta_graph_error_auth_failure_code_190() {
    let err_json = json!({
        "error": {
            "message": "Error validating access token: Session has expired on Monday, 24-Aug-26 09:00:00 PDT.",
            "type": "OAuthException",
            "code": 190,
            "error_subcode": 463,
            "fbtrace_id": "AbCdEf123"
        }
    });

    let err = super::error::MetaGraphError::parse(400, &err_json);
    assert_eq!(err.kind, super::error::MetaErrorKind::AuthFailure);
    assert!(err.is_auth_failure());
    assert!(!err.is_transient());
    assert_eq!(err.code, Some(190));
    assert_eq!(err.subcode, Some(463));
    assert!(err.to_string().contains("code 190"));
}

#[test]
fn test_parse_meta_graph_error_http_401() {
    let err_json = json!({
        "error": {
            "message": "The access token could not be decrypted",
            "type": "OAuthException",
            "code": 102
        }
    });

    let err = super::error::MetaGraphError::parse(401, &err_json);
    assert_eq!(err.kind, super::error::MetaErrorKind::AuthFailure);
    assert!(err.is_auth_failure());
}

#[test]
fn test_parse_meta_graph_error_transient() {
    let err_json = json!({
        "error": {
            "message": "An unexpected error has occurred. Please retry your request later.",
            "type": "OAuthException",
            "code": 2,
            "is_transient": true
        }
    });

    let err = super::error::MetaGraphError::parse(500, &err_json);
    assert_eq!(err.kind, super::error::MetaErrorKind::Transient);
    assert!(err.is_transient());
    assert!(!err.is_auth_failure());
}

#[test]
fn test_parse_meta_graph_error_policy_violation() {
    let err_json = json!({
        "error": {
            "message": "Message failed to send because more than 24 hours have passed since the recipient last interacted with the page.",
            "type": "OAuthException",
            "code": 2018278
        }
    });

    let err = super::error::MetaGraphError::parse(400, &err_json);
    assert_eq!(err.kind, super::error::MetaErrorKind::PolicyViolation);
    assert!(!err.is_auth_failure());
    assert!(!err.is_transient());
}
