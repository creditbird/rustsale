use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use super::attachment::MediaFetcher;
use super::clients::{sha256, ClientStore};
use super::handlers::ApiState;
use super::rate_limit::RateLimiter;
use super::server::build_router;
use crate::conversation::commands_test::{direct_message, ServiceHarness};
use crate::conversation::service::ConversationService;
use crate::conversation::zalo::client::RealZaloClientFactory;
use crate::conversation::zalo::supervisor::ZaloSessionSupervisor;

const TOKEN: &str = "test-token-123";
const READER_TOKEN: &str = "reader-token-456";

struct Harness {
    _service_harness: ServiceHarness,
    router: axum::Router,
    clients_file: std::path::PathBuf,
}

fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn write_clients(path: &std::path::Path, disabled: bool) {
    let body = serde_json::json!([{
        "name": "reader",
        "sha256": hex(sha256(READER_TOKEN)),
        "scopes": ["inbox"],
        "disabled": disabled,
    }]);
    std::fs::write(path, serde_json::to_vec(&body).unwrap()).unwrap();
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
    let clients_file = service_harness
        .path
        .parent()
        .unwrap()
        .join(format!("clients-{}.json", rand::random::<u64>()));
    write_clients(&clients_file, false);
    let api_state = ApiState {
        conversation: Arc::new(conv_state),
        clients: Arc::new(ClientStore::new(Some(clients_file.clone()), Some(TOKEN))),
        limiter: Arc::new(RateLimiter::default()),
        // Loopback is allowed so tests can serve attachments from a fake CDN.
        media: Arc::new(MediaFetcher::new(true, FAKE_LIMIT)),
    };
    let router = build_router(api_state);
    Harness {
        _service_harness: service_harness,
        router,
        clients_file,
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

async fn raw(
    router: &axum::Router,
    request: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn mcp(router: &axum::Router, token: &str, body: Value) -> (StatusCode, Value) {
    send(router, "POST", "/mcp", Some(token), Some(body)).await
}

fn tool_payload(body: &Value) -> Value {
    serde_json::from_str(body["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn framework_errors_are_json() {
    let harness = build_harness();
    let (status, body) = send(&harness.router, "GET", "/nope", Some(TOKEN), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "not_found");

    let (status, body) = send(&harness.router, "GET", "/mcp", Some(TOKEN), None).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(body["code"], "method_not_allowed");

    let (status, body) = send(
        &harness.router,
        "GET",
        "/api/conversations/abc/timeline",
        Some(TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "bad_request");

    let request = Request::builder()
        .method("POST")
        .uri("/api/conversations/1/messages")
        .header("authorization", format!("Bearer {TOKEN}"))
        .header("content-type", "text/plain")
        .body(Body::from("hi"))
        .unwrap();
    let (status, _, body) = raw(&harness.router, request).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(body["code"], "unsupported_media_type");

    let (status, body) = send(
        &harness.router,
        "POST",
        "/api/conversations/1/messages",
        Some(TOKEN),
        Some(serde_json::json!({"nope": 1})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "bad_request");

    let (status, body) = send(
        &harness.router,
        "GET",
        "/api/inbox?limit=0&view=compact",
        Some(TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "bad_request");
}

#[tokio::test]
async fn file_client_scopes_and_hot_revoke() {
    let harness = build_harness();
    let (status, _) = send(
        &harness.router,
        "GET",
        "/api/inbox",
        Some(READER_TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = send(
        &harness.router,
        "GET",
        "/api/conversations/1/timeline",
        Some(READER_TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "forbidden");
    let (_, body) = mcp(
        &harness.router,
        READER_TOKEN,
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"mark_read","arguments":{"id":1}}}),
    )
    .await;
    assert_eq!(body["result"]["isError"], true);
    assert_eq!(tool_payload(&body)["code"], "forbidden");

    write_clients(&harness.clients_file, true);
    let future = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
    std::fs::File::options()
        .write(true)
        .open(&harness.clients_file)
        .unwrap()
        .set_modified(future)
        .unwrap();
    let (status, _) = send(
        &harness.router,
        "GET",
        "/api/inbox",
        Some(READER_TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // Legacy env token keeps working.
    let (status, _) = send(&harness.router, "GET", "/api/inbox", Some(TOKEN), None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn rate_limits_requests_and_sends() {
    let harness = build_harness();
    for _ in 0..10 {
        let (status, _) = send(
            &harness.router,
            "POST",
            "/api/conversations/999999/messages",
            Some(TOKEN),
            Some(serde_json::json!({"text": "x"})),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
    let (status, body) = mcp(
        &harness.router,
        TOKEN,
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"send_message","arguments":{"id":999999,"text":"x"}}}),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["code"], "rate_limited");

    for _ in 0..49 {
        send(
            &harness.router,
            "GET",
            "/api/inbox?view=compact",
            Some(TOKEN),
            None,
        )
        .await;
    }
    let request = Request::builder()
        .uri("/api/inbox")
        .header("authorization", format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap();
    let (status, headers, _) = raw(&harness.router, request).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert!(headers.get("retry-after").is_some());
}

#[tokio::test]
async fn mcp_protocol_basics() {
    let harness = build_harness();
    let (status, body) = mcp(
        &harness.router,
        TOKEN,
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"]["protocolVersion"], "2025-03-26");
    assert!(body["result"]["capabilities"]["tools"].is_object());

    let (status, body) = mcp(
        &harness.router,
        TOKEN,
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(body, Value::Null);

    let (_, body) = mcp(
        &harness.router,
        TOKEN,
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
    )
    .await;
    let names: Vec<&str> = body["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "inbox_unread",
            "read_timeline",
            "mark_read",
            "send_message",
            "read_attachment"
        ]
    );

    let (_, body) = mcp(
        &harness.router,
        TOKEN,
        serde_json::json!({"jsonrpc":"2.0","id":3,"method":"nope"}),
    )
    .await;
    assert_eq!(body["error"]["code"], -32601);
    let (_, body) = mcp(
        &harness.router,
        TOKEN,
        serde_json::json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"zzz"}}),
    )
    .await;
    assert_eq!(body["error"]["code"], -32602);

    let request = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("authorization", format!("Bearer {TOKEN}"))
        .header("content-type", "application/json")
        .body(Body::from("{oops"))
        .unwrap();
    let (_, _, body) = raw(&harness.router, request).await;
    assert_eq!(body["error"]["code"], -32700);

    let (_, body) = mcp(
        &harness.router,
        TOKEN,
        serde_json::json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"inbox_unread","arguments":{}}}),
    )
    .await;
    assert!(body["result"]["isError"].is_null());
    assert_eq!(tool_payload(&body), serde_json::json!([]));

    let (_, body) = mcp(
        &harness.router,
        TOKEN,
        serde_json::json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"send_message","arguments":{"id":999999,"text":"x"}}}),
    )
    .await;
    assert_eq!(body["result"]["isError"], true);
    assert_eq!(tool_payload(&body)["code"], "not_found");

    let request = Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("authorization", format!("Bearer {TOKEN}"))
        .header("origin", "https://evil.example")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#))
        .unwrap();
    let (status, _, _) = raw(&harness.router, request).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn compact_timeline_and_cursor() {
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
            "/api/conversations/{}/timeline?view=compact",
            applied.conversation_id
        ),
        Some(TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"][0]["dir"], "in");
    assert!(body["items"][0].get("st").is_none());

    let (status, body) = send(
        &harness.router,
        "GET",
        &format!(
            "/api/conversations/{}/timeline?view=compact&before=%%%",
            applied.conversation_id
        ),
        Some(TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let cursor = crate::conversation::types::TimelineCursor {
        occurred_at: "2026-01-01T00:00:00Z".into(),
        message_id: 42,
    };
    let decoded = super::intake::decode_cursor(&super::intake::encode_cursor(&cursor)).unwrap();
    assert_eq!(decoded.message_id, 42);
    assert_eq!(decoded.occurred_at, cursor.occurred_at);
}

#[test]
fn compact_inbox_skips_groups_and_read_chats() {
    use crate::conversation::types::{InboxItem, InboxItemKind};
    let item = |kind, conversation_id, unread, at: &str| InboxItem {
        key: format!("conversation:{conversation_id}"),
        kind,
        lead_id: None,
        conversation_id: Some(conversation_id),
        identity_id: None,
        title: format!("c{conversation_id}"),
        avatar_url: None,
        last_message_preview: Some("x".repeat(200)),
        last_message_at: Some(at.to_string()),
        unread_count: unread,
        channels: Vec::new(),
        reply_targets: Vec::new(),
        participant_count: 0,
        participants: Vec::new(),
    };
    let out = super::intake::compact_inbox(
        vec![
            item(InboxItemKind::Customer, 1, 2, "2026-01-01"),
            item(InboxItemKind::Group, 2, 5, "2026-01-03"),
            item(InboxItemKind::Unknown, 3, 1, "2026-01-02"),
            item(InboxItemKind::Customer, 4, 0, "2026-01-04"),
        ],
        20,
    );
    let ids: Vec<i64> = out.iter().map(|e| e.id).collect();
    assert_eq!(ids, [3, 1]);
    assert_eq!(
        out[0].preview.as_deref().map(|p| p.chars().count()),
        Some(81)
    );
}

#[test]
fn conversation_errors_map_to_http_status() {
    use super::error::ApiError;
    use crate::conversation::error::{ConversationError, ConversationErrorCode as C};
    let map = |code| ApiError::from(ConversationError::new(code, "detail"));
    assert_eq!(map(C::InvalidRequest).status, StatusCode::BAD_REQUEST);
    assert_eq!(map(C::NotFound).status, StatusCode::NOT_FOUND);
    assert_eq!(map(C::AccountDisconnected).status, StatusCode::CONFLICT);
    assert_eq!(
        map(C::ProviderRateLimited).status,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(map(C::ProviderAuth).status, StatusCode::BAD_GATEWAY);
    let internal = map(C::ProviderFailure);
    assert_eq!(internal.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(internal.message, "internal error");
}

// ---- read_attachment -------------------------------------------------------------------

const FAKE_LIMIT: u64 = 1024;
/// Tiny PNG.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

/// Fake CDN on a random loopback port; returns its base URL (`http://127.0.0.1:port`).
async fn fake_cdn() -> String {
    use axum::http::header::CONTENT_TYPE;
    use axum::routing::get;
    let app = axum::Router::new()
        .route(
            "/ok.png",
            get(|| async { ([(CONTENT_TYPE, "image/png")], PNG.to_vec()) }),
        )
        .route("/gone.png", get(|| async { StatusCode::NOT_FOUND }))
        .route("/denied.png", get(|| async { StatusCode::FORBIDDEN }))
        .route(
            "/boom.png",
            get(|| async { StatusCode::INTERNAL_SERVER_ERROR }),
        )
        .route(
            "/huge.png",
            get(|| async { ([(CONTENT_TYPE, "image/png")], vec![7u8; 2048]) }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    format!("http://{addr}")
}

fn attachment(
    media_type: &str,
    remote_url: Option<String>,
    thumbnail_url: Option<String>,
    size_bytes: Option<u64>,
) -> crate::conversation::repository::NormalizedAttachment {
    crate::conversation::repository::NormalizedAttachment {
        provider_attachment_id: None,
        media_type: media_type.to_string(),
        mime_type: if media_type == "image" {
            "image/png".to_string()
        } else {
            "application/pdf".to_string()
        },
        file_name: None,
        size_bytes,
        remote_url,
        thumbnail_url,
        metadata_json: Some(serde_json::json!({})),
    }
}

/// Stores one inbound message with one attachment; returns (conversation id, attachment id).
fn seed(
    harness: &Harness,
    key: &str,
    attachment: crate::conversation::repository::NormalizedAttachment,
) -> (i64, i64) {
    let mut message = direct_message("imei-local-api", key);
    message.content_type = "image".to_string();
    message.attachments = vec![attachment];
    let applied = harness
        ._service_harness
        .repository
        .apply_normalized_message(&message)
        .unwrap();
    let refs = harness
        ._service_harness
        .repository
        .list_message_attachment_refs(applied.message_id)
        .unwrap();
    assert_eq!(refs.len(), 1);
    (applied.conversation_id, refs[0].0)
}

async fn mcp_read_attachment(harness: &Harness, token: &str, id: i64) -> Value {
    let (status, body) = mcp(
        &harness.router,
        token,
        serde_json::json!({"jsonrpc":"2.0","id":9,"method":"tools/call",
            "params":{"name":"read_attachment","arguments":{"attachment_id": id}}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    body
}

#[tokio::test]
async fn compact_timeline_lists_attachment_ids_and_keeps_legacy_att() {
    let harness = build_harness();
    let (conversation, attachment_id) = seed(
        &harness,
        "m-ids",
        attachment(
            "image",
            Some("https://cdn.example/a.png".into()),
            None,
            None,
        ),
    );
    let (status, body) = send(
        &harness.router,
        "GET",
        &format!("/api/conversations/{conversation}/timeline?view=compact"),
        Some(TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let item = &body["items"][0];
    assert_eq!(item["att"], serde_json::json!(["image"]));
    assert_eq!(
        item["atts"],
        serde_json::json!([{"id": attachment_id, "kind": "image", "mime": "image/png"}])
    );
    assert!(
        !body.to_string().contains("cdn.example"),
        "compact timeline must not expose provider URLs"
    );
}

#[tokio::test]
async fn read_attachment_over_mcp_returns_the_image_as_base64() {
    use base64::Engine as _;
    let cdn = fake_cdn().await;
    let harness = build_harness();
    let (_, id) = seed(
        &harness,
        "m-ok",
        attachment("image", Some(format!("{cdn}/ok.png")), None, None),
    );
    let body = mcp_read_attachment(&harness, TOKEN, id).await;
    let content = &body["result"]["content"][0];
    assert_eq!(content["type"], "image");
    assert_eq!(content["mimeType"], "image/png");
    assert_eq!(
        content["data"],
        base64::engine::general_purpose::STANDARD.encode(PNG)
    );
    assert!(body["result"].get("isError").is_none());
}

#[tokio::test]
async fn read_attachment_over_rest_returns_raw_bytes_with_mime() {
    let cdn = fake_cdn().await;
    let harness = build_harness();
    let (_, id) = seed(
        &harness,
        "m-rest",
        attachment("image", Some(format!("{cdn}/ok.png")), None, None),
    );
    let request = Request::builder()
        .uri(format!("/api/attachments/{id}"))
        .header("authorization", format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap();
    let response = harness.router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "image/png");
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(bytes.as_ref(), PNG);
}

#[tokio::test]
async fn read_attachment_uses_inline_thumbnail_when_there_is_no_remote_url() {
    use base64::Engine as _;
    let harness = build_harness();
    let data_url = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(PNG)
    );
    let (_, id) = seed(
        &harness,
        "m-thumb",
        attachment("image", None, Some(data_url), None),
    );
    let body = mcp_read_attachment(&harness, TOKEN, id).await;
    assert_eq!(body["result"]["content"][0]["type"], "image");
    assert_eq!(
        body["result"]["content"][0]["data"],
        base64::engine::general_purpose::STANDARD.encode(PNG)
    );
}

#[tokio::test]
async fn expired_cdn_links_report_link_expired_without_leaking_the_url() {
    let cdn = fake_cdn().await;
    let harness = build_harness();
    for (key, path) in [("m-404", "gone.png"), ("m-403", "denied.png")] {
        let url = format!("{cdn}/{path}");
        let (_, id) = seed(
            &harness,
            key,
            attachment("image", Some(url.clone()), None, None),
        );
        let body = mcp_read_attachment(&harness, TOKEN, id).await;
        assert_eq!(body["result"]["isError"], true);
        let error = tool_payload(&body);
        assert_eq!(error["code"], "LINK_EXPIRED");
        assert!(!error.to_string().contains(&url));

        let (status, rest) = send(
            &harness.router,
            "GET",
            &format!("/api/attachments/{id}"),
            Some(TOKEN),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::GONE);
        assert_eq!(rest["code"], "LINK_EXPIRED");
    }
}

#[tokio::test]
async fn cdn_server_error_reports_download_failed() {
    let cdn = fake_cdn().await;
    let harness = build_harness();
    let (_, id) = seed(
        &harness,
        "m-500",
        attachment("image", Some(format!("{cdn}/boom.png")), None, None),
    );
    let error = tool_payload(&mcp_read_attachment(&harness, TOKEN, id).await);
    assert_eq!(error["code"], "DOWNLOAD_FAILED");
    let (status, _) = send(
        &harness.router,
        "GET",
        &format!("/api/attachments/{id}"),
        Some(TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn attachment_without_any_source_reports_download_failed() {
    let harness = build_harness();
    let (_, id) = seed(&harness, "m-nosrc", attachment("image", None, None, None));
    let error = tool_payload(&mcp_read_attachment(&harness, TOKEN, id).await);
    assert_eq!(error["code"], "DOWNLOAD_FAILED");
}

#[tokio::test]
async fn non_image_attachment_reports_unsupported_kind() {
    let harness = build_harness();
    let (_, id) = seed(
        &harness,
        "m-file",
        attachment("file", Some("https://cdn.example/x.pdf".into()), None, None),
    );
    let error = tool_payload(&mcp_read_attachment(&harness, TOKEN, id).await);
    assert_eq!(error["code"], "UNSUPPORTED_KIND");
    let (status, _) = send(
        &harness.router,
        "GET",
        &format!("/api/attachments/{id}"),
        Some(TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn unknown_attachment_id_reports_attachment_not_found() {
    let harness = build_harness();
    let error = tool_payload(&mcp_read_attachment(&harness, TOKEN, 987_654).await);
    assert_eq!(error["code"], "ATTACHMENT_NOT_FOUND");
    let (status, body) = send(
        &harness.router,
        "GET",
        "/api/attachments/987654",
        Some(TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "ATTACHMENT_NOT_FOUND");
}

#[tokio::test]
async fn oversized_attachment_reports_too_large_declared_or_streamed() {
    let cdn = fake_cdn().await;
    let harness = build_harness();
    // Declared size already over the limit: rejected before any download.
    let (_, declared) = seed(
        &harness,
        "m-big-declared",
        attachment(
            "image",
            Some(format!("{cdn}/ok.png")),
            None,
            Some(FAKE_LIMIT + 1),
        ),
    );
    // Declared size lies (None): the body itself is over the limit.
    let (_, streamed) = seed(
        &harness,
        "m-big-streamed",
        attachment("image", Some(format!("{cdn}/huge.png")), None, None),
    );
    for id in [declared, streamed] {
        let error = tool_payload(&mcp_read_attachment(&harness, TOKEN, id).await);
        assert_eq!(error["code"], "TOO_LARGE");
        let (status, _) = send(
            &harness.router,
            "GET",
            &format!("/api/attachments/{id}"),
            Some(TOKEN),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    }
}

#[tokio::test]
async fn read_attachment_needs_the_timeline_scope() {
    let harness = build_harness();
    let (_, id) = seed(&harness, "m-scope", attachment("image", None, None, None));
    let (status, body) = send(
        &harness.router,
        "GET",
        &format!("/api/attachments/{id}"),
        Some(READER_TOKEN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "forbidden");
    let body = mcp_read_attachment(&harness, READER_TOKEN, id).await;
    assert_eq!(body["result"]["isError"], true);
    assert_eq!(tool_payload(&body)["code"], "forbidden");
}

#[tokio::test]
async fn private_hosts_are_not_fetched_in_production_mode() {
    let cdn = fake_cdn().await;
    let strict = MediaFetcher::new(false, FAKE_LIMIT);
    let outcome = strict.fetch(&format!("{cdn}/ok.png")).await;
    assert_eq!(outcome.err().map(|e| e.code), Some("DOWNLOAD_FAILED"));
}
