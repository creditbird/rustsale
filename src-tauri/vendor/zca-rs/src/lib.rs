//! zca-rs is a Rust port of the unofficial Zalo client library `zca-js`.
//!
//! It provides 100% safe, async interface to Zalo APIs, including cryptographic helpers,
//! connection listener, and comprehensive API endpoints.

#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::await_holding_lock,
    clippy::indexing_slicing
)]

pub mod apis;
pub mod context;
pub mod crypto;
pub mod error;
pub mod listener;
pub mod models;

pub use apis::Zalo;
pub use apis::profile::ZaloAccountProfile;
pub use context::{Context, ImageMetadataGetter, ImageMetadataGetterResponse, Options};
pub use error::ZaloError;
