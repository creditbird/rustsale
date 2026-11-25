use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::Router;

use super::attachment::MediaFetcher;
use super::auth::{guard, json_errors, MAX_BODY_BYTES};
use super::clients::ClientStore;
use super::handlers::{get_attachment, get_inbox, get_timeline, mark_read, send_message, ApiState};
use super::mcp;
use super::rate_limit::RateLimiter;
use crate::conversation::commands::ConversationState;

const DEFAULT_PORT: u16 = 8787;
const TOKEN_ENV_VAR: &str = "RUSTSALE_LOCAL_API_TOKEN";
const PORT_ENV_VAR: &str = "RUSTSALE_LOCAL_API_PORT";
const CLIENTS_ENV_VAR: &str = "RUSTSALE_LOCAL_API_CLIENTS_FILE";
const CLIENTS_FILE_NAME: &str = "local-api-clients.json";

pub fn build_router(state: ApiState) -> Router {
    Router::new()
        .route("/api/inbox", get(get_inbox))
        .route("/api/conversations/{id}/timeline", get(get_timeline))
        .route("/api/attachments/{id}", get(get_attachment))
        .route("/api/conversations/{id}/read", post(mark_read))
        .route("/api/conversations/{id}/messages", post(send_message))
        .route("/mcp", post(mcp::handle))
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), guard))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(axum::middleware::from_fn(json_errors))
        .with_state(state)
}

/// Clients file: env override, else next to the app database.
fn clients_path(conversation: &ConversationState) -> Option<PathBuf> {
    if let Ok(path) = std::env::var(CLIENTS_ENV_VAR) {
        if !path.trim().is_empty() {
            return Some(PathBuf::from(path.trim()));
        }
    }
    std::path::Path::new(conversation.service.repository.db_path())
        .parent()
        .map(|dir| dir.join(CLIENTS_FILE_NAME))
}

/// Starts the local HTTP API on 127.0.0.1 only. Clients authenticate with a
/// per-client token (hashed in the clients file) or the legacy env token.
pub fn spawn(conversation: Arc<ConversationState>, port_override: Option<u16>) {
    let legacy = std::env::var(TOKEN_ENV_VAR).ok();
    let legacy = legacy.as_deref().filter(|v| !v.trim().is_empty());
    if legacy.is_some() {
        tracing::warn!(
            target: "local_api",
            "{TOKEN_ENV_VAR} in use as client 'legacy-env' (full access); migrate to per-client tokens"
        );
    }
    let path = clients_path(&conversation);
    let clients_file_exists = path.as_ref().is_some_and(|p| p.exists());
    if legacy.is_none() && !clients_file_exists {
        tracing::warn!(target: "local_api", "no clients file and {TOKEN_ENV_VAR} not set; local HTTP API disabled");
        return;
    }

    let port = port_override.unwrap_or_else(|| {
        std::env::var(PORT_ENV_VAR)
            .ok()
            .and_then(|value| value.trim().parse::<u16>().ok())
            .unwrap_or(DEFAULT_PORT)
    });

    let state = ApiState {
        conversation,
        clients: Arc::new(ClientStore::new(path, legacy)),
        limiter: Arc::new(RateLimiter::default()),
        media: Arc::new(MediaFetcher::production()),
    };
    let router = build_router(state);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    tokio::spawn(async move {
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
