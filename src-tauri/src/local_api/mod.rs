//! Local-only HTTP API (REST + minimal MCP) over the conversation module, for
//! automation tooling on the same machine. Bound to 127.0.0.1 only; see
//! `server::spawn` and docs/local-api.md.

pub mod attachment;
pub mod auth;
pub mod clients;
pub mod error;
pub mod handlers;
pub mod intake;
pub mod mcp;
pub mod rate_limit;
pub mod server;

#[cfg(test)]
mod local_api_test;
