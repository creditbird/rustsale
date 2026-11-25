//! Managed Zalo sessions: QR login, account list, session checks and deletion.

use crate::error::CoreApiError;
use crate::runtime::{on_blocking, on_core, state_of};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::commands::{self, DbState};
use rustsale_core::conversation::commands::{self as conversation, ConversationState};

/// Managed Zalo accounts and their sessions. The QR login is a long job: it answers once the
/// login session ends and reports each step through `zalo-qr-*` events on `Host.onEvent`.
#[module]
pub struct Zalo;

#[module]
impl Zalo {
    /// Starts a Zalo QR login. `zalo-qr-code`, `zalo-qr-scanned`, `zalo-qr-expired`,
    /// `zalo-qr-declined`, `zalo-qr-success` and `zalo-qr-error` arrive as events; resolves when
    /// the login ends (success, refusal or an error).
    pub async fn start_qr_login() -> Result<JsonValue, JsError> {
        on_core(move |h| async move {
            let app = h.clone();
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::start_zalo_qr_login(app, state).await?)
        })
        .await
    }

    /// Every managed Zalo account with its auth status, runtime status and last error.
    pub async fn accounts() -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_managed_zalo_accounts(state)?)
        })
        .await
    }

    /// Stops the listener of session `imei` and deletes the session and its account.
    pub async fn delete_session(imei: String) -> Result<JsonValue, JsError> {
        on_core(move |h| async move {
            let app = h.clone();
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_zalo_session(app, imei, state).await?)
        })
        .await
    }

    /// Checks session `imei` against Zalo and resolves `{ imei, is_valid, display_name, avatar,
    /// error_message }`. A missing session rejects with `not_found`.
    pub async fn verify_session(imei: String) -> Result<JsonValue, JsError> {
        on_core(move |h| async move {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::verify_zalo_session(imei, state).await?)
        })
        .await
    }

    /// Restarts the listener of account `accountKey` (the v2 `reconnect_zalo_account`); resolves
    /// once it is connected again and rejects with the core's reason otherwise.
    pub async fn reconnect(account_key: String) -> Result<JsonValue, JsError> {
        on_core(move |h| async move {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(
                conversation::restart_conversation_account(state, account_key).await?,
            )
        })
        .await
    }
}
