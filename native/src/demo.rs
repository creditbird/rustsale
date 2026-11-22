//! Synthetic data for screen tests: a throwaway data folder only.

use crate::error::CoreApiError;
use crate::runtime::{on_blocking, running, state_of};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::commands::DbState;
use rustsale_core::demo_seed::{seed_demo_conversations, seed_demo_scrape};
use std::sync::PoisonError;

/// Refuses any data folder that is not under the system temp folder.
fn check_throwaway() -> Result<(), CoreApiError> {
    let running = running()?;
    if running.data_dir().starts_with(std::env::temp_dir()) {
        Ok(())
    } else {
        Err(CoreApiError::invalid(
            "demo data may only be seeded into a folder under the system temp folder",
        ))
    }
}

/// Seeds synthetic conversations and scrape results. Works only on a data folder under the
/// system temp folder, so a real data folder can never receive it.
#[module]
pub struct Demo;

#[module]
impl Demo {
    /// Inserts one account, three conversations (the longest has 30 messages) and two scrape jobs
    /// with places. Resolves `{ longConversationId, longConversationMessages }`.
    pub async fn seed() -> Result<JsonValue, JsError> {
        check_throwaway()?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            let conn = state.conn.lock().unwrap_or_else(PoisonError::into_inner);
            // One process serves several test files on one folder: a second call finds the data in place.
            let seeded: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM channel_accounts WHERE provider_account_key = 'demo-account'",
                    [],
                    |r| r.get(0),
                )
                .map_err(|e| CoreApiError::internal(format!("cannot read demo state: {e}")))?;
            if seeded > 0 {
                return Ok::<_, CoreApiError>(serde_json::json!({
                    "longConversationId": 1,
                    "longConversationMessages": 30,
                }));
            }
            let seed = seed_demo_conversations(&conn)
                .map_err(|e| CoreApiError::internal(format!("cannot seed conversations: {e}")))?;
            seed_demo_scrape(&conn, 25)
                .map_err(|e| CoreApiError::internal(format!("cannot seed places: {e}")))?;
            Ok::<_, CoreApiError>(serde_json::json!({
                "longConversationId": seed.long_conversation_id,
                "longConversationMessages": seed.long_conversation_messages,
            }))
        })
        .await
    }
}
