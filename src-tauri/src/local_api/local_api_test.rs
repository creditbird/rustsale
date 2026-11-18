use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use super::handlers::ApiState;
use super::server::build_router;
use crate::conversation::commands_test::{direct_message, ServiceHarness};
use crate::conversation::service::ConversationService;
use crate::conversation::zalo::client::RealZaloClientFactory;
use crate::conversation::zalo::supervisor::ZaloSessionSupervisor;

const TOKEN: &str = "test-token-123";

struct Harness {
    _service_harness: ServiceHarness,
    router: axum::Router,
}

fn build_harness() -> Harness {
    let service_harness = ServiceHarness::new();
    let conversation_service = Arc::new(ConversationService::new(
        service_harness.repository.clone(),
        service_harness.events.clone(),
        service_harness
            .path
            .parent()
            .unwrap()
            .join("spool-local-api"),
    ));
    let zalo_supervisor = Arc::new(ZaloSessionSupervisor::new(
        service_harness.path.to_str().unwrap().to_string(),
        conversation_service.clone(),
        Arc::new(RealZaloClientFactory),
    ));
    let conv_state = service_harness.create_conv_state(conversation_service, zalo_supervisor);
    let api_state = ApiState {
        conversation: Arc::new(conv_state),
        token: Arc::from(TOKEN),
    };
    let router = build_router(api_state);
    Harness {
        _service_harness: service_harness,
        router,
    }
}

async fn send(
    router: &axum::Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let request = if let Some(body) = body {
        builder
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    } else {
        builder.body(Body::empty()).unwrap()
    };

    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, json)
}

#[tokio::test]
async fn missing_bearer_token_is_rejected() {
    let harness = build_harness();
    let (status, _) = send(&harness.router, "GET", "/api/inbox", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn wrong_bearer_token_is_rejected() {
    let harness = build_harness();
    let (status, _) = send(
        &harness.router,
        "GET",
        "/api/inbox",
        Some("not-the-token"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_inbox_with_valid_token_returns_ok() {
    let harness = build_harness();
    let (status, body) = send(&harness.router, "GET", "/api/inbox", Some(TOKEN), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn timeline_for_unknown_conversation_is_not_found() {
    let harness = build_harness();
    let (status, body) = send(
        &harness.router,
        "GET",
        "/api/conversations/999999/timeline?limit=10",
        Some(TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "not_found");
}

#[tokio::test]
async fn mark_read_for_unknown_conversation_is_not_found() {
    let harness = build_harness();
    let (status, _) = send(
        &harness.router,
        "POST",
        "/api/conversations/999999/read",
        Some(TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn send_message_with_empty_text_is_bad_request() {
    let harness = build_harness();
    let (status, body) = send(
        &harness.router,
        "POST",
        "/api/conversations/1/messages",
        Some(TOKEN),
        Some(serde_json::json!({ "channelAccountId": 1, "text": "   " })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "bad_request");
}

#[tokio::test]
async fn send_message_malformed_body_is_bad_request() {
    let harness = build_harness();
    let request = Request::builder()
        .method("POST")
        .uri("/api/conversations/1/messages")
        .header("authorization", format!("Bearer {TOKEN}"))
        .header("content-type", "application/json")
        .body(Body::from("{not valid json"))
        .unwrap();
    let response = harness.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn timeline_and_read_succeed_for_existing_conversation() {
    let harness = build_harness();
    let applied = harness
        ._service_harness
        .repository
        .apply_normalized_message(&direct_message("imei-local-api", "message-1"))
        .unwrap();

    let (status, body) = send(
        &harness.router,
        "GET",
        &format!(
            "/api/conversations/{}/timeline?limit=10",
            applied.conversation_id
        ),
        Some(TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"].as_array().unwrap().len(), 1);

    let (status, body) = send(
        &harness.router,
        "POST",
        &format!("/api/conversations/{}/read", applied.conversation_id),
        Some(TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["rowsMarked"], 1);
}
