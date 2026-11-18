use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use super::handlers::ApiState;

#[derive(Serialize)]
struct AuthErrorBody {
    code: &'static str,
    message: &'static str,
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(AuthErrorBody {
            code: "unauthorized",
            message: "missing or invalid bearer token",
        }),
    )
        .into_response()
}

/// Avoids leaking token length/content through response-time differences.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

pub async fn require_bearer_token(
    State(state): State<ApiState>,
    request: Request,
    next: Next,
) -> Response {
    let provided = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));

    match provided {
        Some(token) if constant_time_eq(token.as_bytes(), state.token.as_bytes()) => {
            next.run(request).await
        }
        _ => unauthorized(),
    }
}
