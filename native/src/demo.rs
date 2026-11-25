//! Synthetic data for screen tests: a throwaway data folder only.

use crate::error::CoreApiError;
use crate::runtime::{on_blocking, running, state_of};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::commands::DbState;
use rustsale_core::demo_seed::{seed_demo_conversations, seed_demo_scrape};
use std::path::{Path, PathBuf};
use std::sync::PoisonError;

/// The file `Demo.prepare` writes into a data folder it made for a test; `Demo.seed` seeds nothing without it.
const MARKER: &str = ".rustsale-demo-sandbox";
/// A name starting with this is the real app's data folder (production or v3-dev): never seeded.
const REAL_DATA: &str = "com.rustsale.crm";

/// Whether `dir` (already resolved) may hold demo data: under the resolved system temp folder, with no
/// path component that names the real app's data folder, however the temp folder was reached.
fn names_real_data(dir: &Path) -> bool {
    dir.components().any(|c| {
        c.as_os_str()
            .to_string_lossy()
            .to_lowercase()
            .starts_with(REAL_DATA)
    })
}

fn is_throwaway(dir: &Path, temp: &Path) -> Result<(), CoreApiError> {
    if names_real_data(dir) {
        return Err(CoreApiError::invalid(
            "demo data is never written to the app's own data folder",
        ));
    }
    if !dir.starts_with(temp) {
        return Err(CoreApiError::invalid(
            "demo data may only be seeded into a folder under the system temp folder",
        ));
    }
    Ok(())
}

fn resolved_temp() -> Result<PathBuf, CoreApiError> {
    std::fs::canonicalize(std::env::temp_dir())
        .map_err(|e| CoreApiError::internal(format!("cannot resolve the temp folder: {e}")))
}

/// Makes `dir` a demo sandbox: a new (or empty) folder under the temp folder that gets the marker.
fn prepare_dir(dir: &Path, temp: &Path) -> Result<(), CoreApiError> {
    // Refuse by the written path first so a bad path creates nothing on disk.
    if names_real_data(dir) {
        return Err(CoreApiError::invalid(
            "demo data is never written to the app's own data folder",
        ));
    }
    std::fs::create_dir_all(dir)
        .map_err(|e| CoreApiError::internal(format!("cannot create the demo folder: {e}")))?;
    let resolved = std::fs::canonicalize(dir)
        .map_err(|e| CoreApiError::internal(format!("cannot resolve the demo folder: {e}")))?;
    is_throwaway(&resolved, temp)?;
    // Only `Demo.prepare` writes the marker, so a folder that has it is one we already made.
    if resolved.join(MARKER).is_file() {
        return Ok(());
    }
    let used = std::fs::read_dir(&resolved)
        .map_err(|e| CoreApiError::internal(format!("cannot read the demo folder: {e}")))?
        .filter_map(Result::ok)
        .next()
        .is_some();
    if used {
        return Err(CoreApiError::invalid(
            "the demo folder must be new or empty",
        ));
    }
    std::fs::write(resolved.join(MARKER), b"demo data only\n")
        .map_err(|e| CoreApiError::internal(format!("cannot mark the demo folder: {e}")))
}

/// Refuses any data folder that is not a marked demo sandbox under the temp folder.
fn check_sandbox(dir: &Path, temp: &Path) -> Result<(), CoreApiError> {
    let resolved = std::fs::canonicalize(dir)
        .map_err(|e| CoreApiError::internal(format!("cannot resolve the data folder: {e}")))?;
    is_throwaway(&resolved, temp)?;
    if resolved.join(MARKER).is_file() {
        Ok(())
    } else {
        Err(CoreApiError::invalid(
            "this data folder was not made for demo data (call Demo.prepare first)",
        ))
    }
}

fn check_throwaway() -> Result<(), CoreApiError> {
    check_sandbox(running()?.data_dir(), &resolved_temp()?)
}

/// Synthetic data for tests and screenshots. Built only with the `demo` feature (or `RUSTSALE_DEMO=1`
/// while building), so a release or `nsh package` build has no such export. Even then it seeds only
/// a folder `Demo.prepare` made under the temp folder.
#[module]
pub struct Demo;

#[module]
impl Demo {
    /// Tells every `Host.onEvent` listener `name` with `payload`, as the core would, so a screen test can
    /// check how the UI reacts to an event without a live channel.
    pub fn emit(name: String, payload: JsonValue) {
        crate::host::deliver_event(&name, &payload);
    }

    /// Makes `dir` (a new or empty folder under the system temp folder) a demo sandbox. Call it before
    /// `Host.start(dir)`. Refuses any path that names the app's own data folder (`com.rustsale.crm*`).
    pub async fn prepare(dir: String) -> Result<(), JsError> {
        let temp = resolved_temp()?;
        Ok(prepare_dir(Path::new(&dir), &temp)?)
    }

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

    /// Adds one expired Zalo account (a session row with no cookies) so a screen test can show an
    /// account row without logging in. Resolves `{ accountKey }`; a second call adds nothing.
    pub async fn seed_zalo_account() -> Result<JsonValue, JsError> {
        check_throwaway()?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            let conn = state.conn.lock().unwrap_or_else(PoisonError::into_inner);
            let seeded: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM zalo_sessions WHERE imei = 'demo-zalo-session'",
                    [],
                    |row| row.get(0),
                )
                .map_err(|e| CoreApiError::internal(format!("cannot read demo state: {e}")))?;
            if seeded == 0 {
                conn.execute(
                    "INSERT INTO zalo_sessions
                     (imei, user_agent, cookies_json, language, display_name, auth_status, zalo_uid)
                     VALUES ('demo-zalo-session', '', '[]', 'vi', 'Zalo demo', 'expired', 'demo-zalo-uid')",
                    [],
                )
                .map_err(|e| CoreApiError::internal(format!("cannot seed the Zalo account: {e}")))?;
            }
            Ok::<_, CoreApiError>(serde_json::json!({ "accountKey": "demo-zalo-uid" }))
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> Result<tempfile::TempDir, std::io::Error> {
        tempfile::tempdir()
    }

    #[test]
    fn a_fresh_folder_is_prepared_and_then_accepted() -> Result<(), Box<dyn std::error::Error>> {
        let root = temp()?;
        let base = std::fs::canonicalize(root.path())?;
        let dir = base.join("demo");
        assert!(check_sandbox(&dir, &base).is_err());
        prepare_dir(&dir, &base)?;
        assert!(check_sandbox(&dir, &base).is_ok());
        // Preparing again (one folder serves several tests) is fine.
        prepare_dir(&dir, &base)?;
        Ok(())
    }

    #[test]
    fn a_folder_that_names_the_real_data_is_refused_even_under_temp()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = temp()?;
        let base = std::fs::canonicalize(root.path())?;
        for name in [
            "com.rustsale.crm",
            "com.rustsale.crm.v3-dev",
            "Com.RustSale.CRM.v3-dev",
        ] {
            let dir = base.join(name);
            assert!(prepare_dir(&dir, &base).is_err(), "{name} must be refused");
            assert!(!dir.exists(), "{name} must not be created");
        }
        let nested = base.join("share").join("com.rustsale.crm.v3-dev").join("x");
        assert!(prepare_dir(&nested, &base).is_err());
        Ok(())
    }

    #[test]
    fn a_temp_folder_moved_onto_the_data_root_changes_nothing()
    -> Result<(), Box<dyn std::error::Error>> {
        // TMPDIR pointed at the folder that holds the real data: the real data folder still names itself.
        let root = temp()?;
        let share = std::fs::canonicalize(root.path())?;
        let real = share.join("com.rustsale.crm.v3-dev");
        std::fs::create_dir_all(&real)?;
        std::fs::write(real.join(MARKER), b"x")?;
        assert!(check_sandbox(&real, &share).is_err());
        Ok(())
    }

    #[test]
    fn an_unmarked_or_used_folder_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let root = temp()?;
        let base = std::fs::canonicalize(root.path())?;
        let used = base.join("used");
        std::fs::create_dir_all(&used)?;
        std::fs::write(used.join("data.db"), b"x")?;
        assert!(check_sandbox(&used, &base).is_err());
        assert!(prepare_dir(&used, &base).is_err());
        Ok(())
    }

    #[test]
    fn a_folder_outside_temp_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let a = temp()?;
        let b = temp()?;
        let outside = std::fs::canonicalize(a.path())?;
        let base = std::fs::canonicalize(b.path())?;
        assert!(prepare_dir(&outside.join("demo"), &base).is_err());
        Ok(())
    }
}
