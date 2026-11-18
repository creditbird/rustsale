//! Local-only HTTP API exposing a read/send surface over the conversation
//! module, for automation tooling (e.g. a chatbot) running on the same
//! machine. Bound to 127.0.0.1 only; see `server::spawn`.

pub mod auth;
pub mod handlers;
pub mod server;

#[cfg(test)]
mod local_api_test;
