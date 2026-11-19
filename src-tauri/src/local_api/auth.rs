//! Request pipeline for matched routes: bearer auth → rate limit → route scope →
//! handler, then one log line. Nothing secret (token, query, body) is logged.

use std::time::Instant;

use axum::body::{to_bytes, Body};
use axum::extract::{MatchedPath, Request, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;

use super::clients::{SCOPE_INBOX, SCOPE_READ, SCOPE_SEND, SCOPE_TIMELINE};
use super::error::{code_for_status, ApiError, ErrorBody};
use super::handlers::ApiState;
use super::mcp::{is_send_call, McpTool};

pub const MAX_BODY_BYTES: usize = 64 * 1024;

/// Scope required by a REST route; `None` for /mcp (checked per tool).
fn route_scope(method: &Method, route: &str) -> Option<&'static str> {
    match (method.as_str(), route) {
        ("GET", "/api/inbox") => Some(SCOPE_INBOX),
        ("GET", "/api/conversations/{id}/timeline") => Some(SCOPE_TIMELINE),
        ("GET", "/api/attachments/{id}") => Some(SCOPE_TIMELINE),
        ("POST", "/api/conversations/{id}/read") => Some(SCOPE_READ),
        ("POST", "/api/conversations/{id}/messages") => Some(SCOPE_SEND),
        _ => None,
    }
}

fn log_line(
    client: &str,
    method: &Method,
    route: &str,
    tool: Option<McpTool>,
    status: StatusCode,
    start: Instant,
) {
    tracing::info!(
        target: "local_api",
        client,
        method = %method,
        route,
        tool = tool.map(|t| t.0).unwrap_or("-"),
        status = status.as_u16(),
        ms = start.elapsed().as_millis() as u64,
        "request"
    );
}

pub async fn guard(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = request.method().clone();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| "-".to_string());

    let client = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .and_then(|token| state.clients.authenticate(token.trim()));
    let Some(client) = client else {
        let response = ApiError::new(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "missing or invalid bearer token",
        )
        .into_response();
        log_line("-", &method, &route, None, response.status(), start);
        return response;
    };

    let (request, is_send) = if route == "/mcp" {
        // Buffer (≤64KB) so the send budget is enforced before the handler runs.
        let (parts, body) = request.into_parts();
        match to_bytes(body, MAX_BODY_BYTES).await {
            Ok(bytes) => {
                let send = is_send_call(&bytes);
                (Request::from_parts(parts, Body::from(bytes)), send)
            }
            Err(_) => {
                let response = ApiError::new(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "payload_too_large",
                    "body too large",
                )
                .into_response();
                log_line(
                    &client.name,
                    &method,
                    &route,
                    None,
                    response.status(),
                    start,
                );
                return response;
            }
        }
    } else {
        let send = route_scope(&method, &route) == Some(SCOPE_SEND);
        (request, send)
    };

    let outcome = if let Err(retry) = state.limiter.check(&client.name, is_send) {
        Err(ApiError::rate_limited(retry))
    } else {
        match route_scope(&method, &route) {
            Some(scope) if !client.has_scope(scope) => {
                Err(ApiError::forbidden(format!("missing scope '{scope}'")))
            }
            _ => Ok(()),
        }
    };
    let response = match outcome {
        Err(error) => error.into_response(),
        Ok(()) => {
            let name = client.name.clone();
            let mut request = request;
            request.extensions_mut().insert(client);
            let response = next.run(request).await;
            let tool = response.extensions().get::<McpTool>().copied();
            log_line(&name, &method, &route, tool, response.status(), start);
            return response;
        }
    };
    log_line(
        &client.name,
        &method,
        &route,
        None,
        response.status(),
        start,
    );
    response
}

/// Rewrites axum's own plain-text error responses (bad JSON, bad path id, 404,
/// 405, 413, 415) into the JSON `{code, message}` shape.
pub async fn json_errors(request: Request, next: Next) -> Response {
    let response = next.run(request).await;
    let status = response.status();
    let is_json = response
        .headers()
        .get(header::CONTENT_TYPE)
        .is_some_and(|v| v.as_bytes().starts_with(b"application/json"));
    if !(status.is_client_error() || status.is_server_error()) || is_json {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    let text = to_bytes(body, 1024).await.unwrap_or_default();
    let detail = String::from_utf8_lossy(&text);
    let message = if detail.trim().is_empty() || status.is_server_error() {
        status
            .canonical_reason()
            .unwrap_or("error")
            .to_ascii_lowercase()
    } else {
        detail.trim().to_string()
    };
    // axum reports a malformed JSON body as 400 and a well-formed one with the
    // wrong shape as 422; both are client data errors → 400.
    let status = if status == StatusCode::UNPROCESSABLE_ENTITY {
        StatusCode::BAD_REQUEST
    } else {
        status
    };
    let body = Json(ErrorBody {
        code: code_for_status(status),
        message: &message,
    })
    .into_response()
    .into_body();
    parts.status = status;
    parts.headers.remove(header::CONTENT_LENGTH);
    parts.headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    Response::from_parts(parts, body)
}
