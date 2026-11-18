use std::net::SocketAddr;
use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;

use super::auth::require_bearer_token;
use super::handlers::{get_inbox, get_timeline, mark_read, send_message, ApiState};
use crate::conversation::commands::ConversationState;

const DEFAULT_PORT: u16 = 8787;
const TOKEN_ENV_VAR: &str = "RUSTSALE_LOCAL_API_TOKEN";
const PORT_ENV_VAR: &str = "RUSTSALE_LOCAL_API_PORT";

pub fn build_router(state: ApiState) -> Router {
    Router::new()
        .route("/api/inbox", get(get_inbox))
        .route("/api/conversations/{id}/timeline", get(get_timeline))
        .route("/api/conversations/{id}/read", post(mark_read))
        .route("/api/conversations/{id}/messages", post(send_message))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_bearer_token,
        ))
        .with_state(state)
}

/// Starts the local HTTP API on the Tauri app's existing async runtime, bound
/// to 127.0.0.1 only. Does nothing (and logs a warning) if no bearer token is
/// configured, since an unauthenticated local API would let any other
/// process on the machine read and send conversation messages.
pub fn spawn(conversation: Arc<ConversationState>) {
    let token = match std::env::var(TOKEN_ENV_VAR) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            tracing::warn!(
                target: "local_api",
                "{TOKEN_ENV_VAR} not set; local HTTP API disabled"
            );
            return;
        }
    };

    let port = std::env::var(PORT_ENV_VAR)
        .ok()
        .and_then(|value| value.trim().parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);

    let state = ApiState {
        conversation,
        token: Arc::from(token.into_boxed_str()),
    };
    let router = build_router(state);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => listener,
            Err(error) => {
                tracing::error!(target: "local_api", %error, %addr, "failed to bind local HTTP API");
                return;
            }
        };
        tracing::info!(target: "local_api", %addr, "local HTTP API listening");
        if let Err(error) = axum::serve(listener, router).await {
            tracing::error!(target: "local_api", %error, "local HTTP API server stopped");
        }
    });
}
