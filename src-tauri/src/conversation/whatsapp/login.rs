use chrono::Utc;
use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::Emitter;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

pub const EVENT_WHATSAPP_QR_CODE: &str = "whatsapp-qr-code";
pub const EVENT_WHATSAPP_LOGIN_SUCCESS: &str = "whatsapp-login-success";
pub const EVENT_WHATSAPP_LOGIN_ERROR: &str = "whatsapp-login-error";

/// Payload for QR code display event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhatsAppQrCodePayload {
    pub attempt_id: String,
    pub qr_raw: String,
    pub expires_in_sec: u32,
    pub sequence: u32,
}

/// Payload for login success event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhatsAppLoginSuccessPayload {
    pub attempt_id: String,
    pub account_id: i64,
    pub account_key: String,
    pub phone_number: String,
    pub push_name: Option<String>,
    pub lid: Option<String>,
    pub avatar_url: Option<String>,
}

/// Payload for login error event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhatsAppLoginErrorPayload {
    pub attempt_id: String,
    pub error: String,
}

/// Internal broadcast event enum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum WhatsAppLoginEvent {
    QrCode(WhatsAppQrCodePayload),
    Success(WhatsAppLoginSuccessPayload),
    Error(WhatsAppLoginErrorPayload),
}

/// State of an in-flight login attempt.
pub struct LoginAttemptState {
    pub attempt_id: String,
    pub temp_db_path: PathBuf,
    pub cancel_token: CancellationToken,
    pub created_at: String,
    pub last_qr_sequence: u32,
    pub is_mock: bool,
    pub task_handle: Option<tokio::task::JoinHandle<()>>,
}

/// Manages WhatsApp QR pairing login attempts, isolated session storage, and callbacks.
#[derive(Clone)]
pub struct WhatsAppLoginManager {
    sessions_dir: PathBuf,
    active_attempts: Arc<Mutex<HashMap<String, LoginAttemptState>>>,
    event_sender: broadcast::Sender<WhatsAppLoginEvent>,
    app_handle: Option<tauri::AppHandle>,
}

impl WhatsAppLoginManager {
    /// Creates a new `WhatsAppLoginManager` with the specified sessions storage directory.
    pub fn new(sessions_dir: PathBuf, app_handle: Option<tauri::AppHandle>) -> Self {
        let (event_sender, _) = broadcast::channel(64);
        Self {
            sessions_dir,
            active_attempts: Arc::new(Mutex::new(HashMap::new())),
            event_sender,
            app_handle,
        }
    }

    /// Creates a mock instance for testing with a temporary directory.
    pub fn new_mock() -> Self {
        let temp_dir =
            std::env::temp_dir().join(format!("wa_mock_sessions_{}", uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&temp_dir);
        Self::new_mock_with_dir(temp_dir)
    }

    /// Creates a mock instance with a specified directory.
    pub fn new_mock_with_dir(sessions_dir: PathBuf) -> Self {
        let (event_sender, _) = broadcast::channel(64);
        Self {
            sessions_dir,
            active_attempts: Arc::new(Mutex::new(HashMap::new())),
            event_sender,
            app_handle: None,
        }
    }

    /// Returns the sessions directory path.
    pub fn sessions_dir(&self) -> &Path {
        &self.sessions_dir
    }

    /// Subscribes to internal login events.
    pub fn subscribe_events(&self) -> broadcast::Receiver<WhatsAppLoginEvent> {
        self.event_sender.subscribe()
    }

    /// Returns the path to the temporary database file for a login attempt.
    pub fn get_temp_db_path(&self, attempt_id: &str) -> PathBuf {
        self.sessions_dir.join(format!("temp_{}.db", attempt_id))
    }

    /// Returns the path to the permanent session database file for a paired account.
    pub fn get_session_db_path(&self, account_key: &str) -> PathBuf {
        self.sessions_dir
            .join(format!("session_{}.db", account_key))
    }

    /// Checks if a login attempt is currently active.
    pub fn is_active(&self, attempt_id: &str) -> bool {
        let attempts = self.active_attempts.lock().unwrap();
        attempts.contains_key(attempt_id)
    }

    /// Returns a list of active attempt IDs.
    pub fn list_active_attempts(&self) -> Vec<String> {
        let attempts = self.active_attempts.lock().unwrap();
        attempts.keys().cloned().collect()
    }

    /// Emits a QR code event to broadcast channel and Tauri app handle (if present).
    pub fn emit_qr_code(&self, attempt_id: &str, qr_raw: &str, expires_in_sec: u32, sequence: u32) {
        let payload = WhatsAppQrCodePayload {
            attempt_id: attempt_id.to_string(),
            qr_raw: qr_raw.to_string(),
            expires_in_sec,
            sequence,
        };

        let _ = self
            .event_sender
            .send(WhatsAppLoginEvent::QrCode(payload.clone()));

        if let Some(app_handle) = &self.app_handle {
            if let Err(err) = app_handle.emit(EVENT_WHATSAPP_QR_CODE, &payload) {
                tracing::warn!(target: "whatsapp_login", %err, "Failed to emit whatsapp-qr-code Tauri event");
            }
        }
    }

    /// Starts a production QR login pairing attempt.
    pub async fn start_qr_login(&self, attempt_id: &str) -> Result<(), String> {
        self.init_attempt_internal(attempt_id, false).await
    }

    /// Starts a mock login attempt for testing without a live WebSocket connection.
    pub async fn start_mock_login(&self, attempt_id: &str) -> Result<(), String> {
        self.init_attempt_internal(attempt_id, true).await
    }

    /// Emits a mock QR event for testing.
    pub async fn emit_mock_qr(
        &self,
        attempt_id: &str,
        qr_raw: &str,
        expires_in_sec: u32,
        sequence: u32,
    ) -> Result<(), String> {
        {
            let mut attempts = self.active_attempts.lock().unwrap();
            let attempt = attempts
                .get_mut(attempt_id)
                .ok_or_else(|| format!("Attempt {} is not active", attempt_id))?;
            attempt.last_qr_sequence = sequence;
        }

        self.emit_qr_code(attempt_id, qr_raw, expires_in_sec, sequence);
        Ok(())
    }

    /// Initiates an attempt state, creates the temporary database file, and registers it.
    async fn init_attempt_internal(&self, attempt_id: &str, is_mock: bool) -> Result<(), String> {
        // If an attempt with this ID already exists, cancel it first
        if self.is_active(attempt_id) {
            let _ = self.cancel_qr_login(attempt_id);
        }

        // Ensure session directory exists
        fs::create_dir_all(&self.sessions_dir)
            .map_err(|e| format!("Failed to create sessions dir: {}", e))?;

        let temp_db_path = self.get_temp_db_path(attempt_id);

        // Touch the temp db file to ensure it exists
        if !temp_db_path.exists() {
            fs::File::create(&temp_db_path)
                .map_err(|e| format!("Failed to create temp session DB file: {}", e))?;
        }

        let cancel_token = CancellationToken::new();
        let state = LoginAttemptState {
            attempt_id: attempt_id.to_string(),
            temp_db_path,
            cancel_token,
            created_at: Utc::now().to_rfc3339(),
            last_qr_sequence: 0,
            is_mock,
            task_handle: None,
        };

        let mut attempts = self.active_attempts.lock().unwrap();
        attempts.insert(attempt_id.to_string(), state);

        Ok(())
    }

    /// Cancels an active login attempt, stops any running tasks, and immediately deletes temporary session files.
    pub fn cancel_qr_login(&self, attempt_id: &str) -> Result<(), String> {
        let attempt = {
            let mut attempts = self.active_attempts.lock().unwrap();
            attempts.remove(attempt_id)
        };

        if let Some(state) = attempt {
            state.cancel_token.cancel();
            if let Some(handle) = state.task_handle {
                handle.abort();
            }

            Self::clean_temp_db_files(&state.temp_db_path);
            Ok(())
        } else {
            // Attempt not found or already cancelled
            let temp_db_path = self.get_temp_db_path(attempt_id);
            Self::clean_temp_db_files(&temp_db_path);
            Ok(())
        }
    }

    /// Cancels all active login attempts and cleans up all temporary files.
    pub fn cancel_all(&self) {
        let attempts: Vec<LoginAttemptState> = {
            let mut map = self.active_attempts.lock().unwrap();
            map.drain().map(|(_, v)| v).collect()
        };

        for state in attempts {
            state.cancel_token.cancel();
            if let Some(handle) = state.task_handle {
                handle.abort();
            }
            Self::clean_temp_db_files(&state.temp_db_path);
        }
    }

    /// Handles a login error: cleans up attempt resources and emits the error event.
    pub fn handle_login_error(&self, attempt_id: &str, error_message: &str) {
        let _ = self.cancel_qr_login(attempt_id);

        let payload = WhatsAppLoginErrorPayload {
            attempt_id: attempt_id.to_string(),
            error: error_message.to_string(),
        };

        let _ = self
            .event_sender
            .send(WhatsAppLoginEvent::Error(payload.clone()));

        if let Some(app_handle) = &self.app_handle {
            if let Err(err) = app_handle.emit(EVENT_WHATSAPP_LOGIN_ERROR, &payload) {
                tracing::warn!(target: "whatsapp_login", %err, "Failed to emit whatsapp-login-error Tauri event");
            }
        }
    }

    /// Finalizes a successful login attempt:
    /// 1. Renames the temporary session DB to `session_<account_key>.db`.
    /// 2. Upserts `channel_accounts` and `whatsapp_sessions` records in the CRM database.
    /// 3. Emits the success event.
    /// 4. Returns the created/updated `channel_account_id`.
    #[allow(clippy::too_many_arguments)]
    pub fn finalize_login_success(
        &self,
        conn: &Connection,
        attempt_id: &str,
        account_key: &str,
        phone_number: &str,
        push_name: Option<&str>,
        lid: Option<&str>,
        avatar_url: Option<&str>,
    ) -> Result<i64, String> {
        let temp_db_path = self.get_temp_db_path(attempt_id);
        let final_db_path = self.get_session_db_path(account_key);

        // Remove active attempt from map
        {
            let mut attempts = self.active_attempts.lock().unwrap();
            if let Some(state) = attempts.remove(attempt_id) {
                state.cancel_token.cancel();
                if let Some(handle) = state.task_handle {
                    handle.abort();
                }
            }
        }

        // Atomically rename/move temp session DB to permanent session DB
        if temp_db_path.exists() {
            if final_db_path.exists() {
                let _ = fs::remove_file(&final_db_path);
            }
            fs::rename(&temp_db_path, &final_db_path).map_err(|e| {
                format!(
                    "Failed to rename session DB from {:?} to {:?}: {}",
                    temp_db_path, final_db_path, e
                )
            })?;

            // Also clean up any leftover temp wal/shm
            Self::clean_temp_db_sidecars(&temp_db_path);
        } else if !final_db_path.exists() {
            // Touch permanent db if neither temp nor final existed
            fs::File::create(&final_db_path)
                .map_err(|e| format!("Failed to create session DB {:?}: {}", final_db_path, e))?;
        }

        let session_db_filename = format!("session_{}.db", account_key);
        let now = Utc::now().to_rfc3339();
        let display_name = push_name
            .map(|s| s.to_string())
            .or_else(|| Some(phone_number.to_string()));

        // Perform atomic CRM database update
        let account_id = (|| -> SqlResult<i64> {
            let tx = conn.unchecked_transaction()?;

            // 1. Upsert channel_accounts
            tx.execute(
                r#"
                INSERT INTO channel_accounts (
                    channel, provider_account_key, display_name, avatar_url,
                    enabled, connection_status, last_connected_at, last_error,
                    capabilities_json, created_at, updated_at
                ) VALUES (
                    'whatsapp', ?1, ?2, ?3,
                    1, 'connected', ?4, NULL,
                    '{}', ?4, ?4
                )
                ON CONFLICT(channel, provider_account_key) DO UPDATE SET
                    display_name = COALESCE(excluded.display_name, channel_accounts.display_name),
                    avatar_url = COALESCE(excluded.avatar_url, channel_accounts.avatar_url),
                    enabled = 1,
                    connection_status = 'connected',
                    last_connected_at = excluded.last_connected_at,
                    last_error = NULL,
                    updated_at = excluded.updated_at
                "#,
                params![account_key, display_name, avatar_url, now],
            )?;

            let account_id: i64 = tx.query_row(
                "SELECT id FROM channel_accounts WHERE channel = 'whatsapp' AND provider_account_key = ?1",
                params![account_key],
                |row| row.get(0),
            )?;

            // 2. Upsert whatsapp_sessions
            tx.execute(
                r#"
                INSERT INTO whatsapp_sessions (
                    channel_account_id, account_key, phone_number, lid, push_name, avatar_url,
                    session_db_filename, is_valid, last_connected_at, last_error, created_at, updated_at
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6,
                    ?7, 1, ?8, NULL, ?8, ?8
                )
                ON CONFLICT(channel_account_id) DO UPDATE SET
                    account_key = excluded.account_key,
                    phone_number = excluded.phone_number,
                    lid = COALESCE(excluded.lid, whatsapp_sessions.lid),
                    push_name = COALESCE(excluded.push_name, whatsapp_sessions.push_name),
                    avatar_url = COALESCE(excluded.avatar_url, whatsapp_sessions.avatar_url),
                    session_db_filename = excluded.session_db_filename,
                    is_valid = 1,
                    last_connected_at = excluded.last_connected_at,
                    last_error = NULL,
                    updated_at = excluded.updated_at
                "#,
                params![
                    account_id,
                    account_key,
                    phone_number,
                    lid,
                    push_name,
                    avatar_url,
                    session_db_filename,
                    now,
                ],
            )?;

            tx.commit()?;
            Ok(account_id)
        })()
        .map_err(|e| format!("Failed to update database for WhatsApp login: {}", e))?;

        // 3. Emit success event
        let payload = WhatsAppLoginSuccessPayload {
            attempt_id: attempt_id.to_string(),
            account_id,
            account_key: account_key.to_string(),
            phone_number: phone_number.to_string(),
            push_name: push_name.map(str::to_string),
            lid: lid.map(str::to_string),
            avatar_url: avatar_url.map(str::to_string),
        };

        let _ = self
            .event_sender
            .send(WhatsAppLoginEvent::Success(payload.clone()));

        if let Some(app_handle) = &self.app_handle {
            if let Err(err) = app_handle.emit(EVENT_WHATSAPP_LOGIN_SUCCESS, &payload) {
                tracing::warn!(target: "whatsapp_login", %err, "Failed to emit whatsapp-login-success Tauri event");
            }
        }

        Ok(account_id)
    }

    /// Helper to remove a temp db file and any SQLite WAL/SHM sidecars.
    fn clean_temp_db_files(temp_db_path: &Path) {
        if temp_db_path.exists() {
            let _ = fs::remove_file(temp_db_path);
        }
        Self::clean_temp_db_sidecars(temp_db_path);
    }

    fn clean_temp_db_sidecars(temp_db_path: &Path) {
        let wal_path1 = temp_db_path.with_extension("db-wal");
        let wal_path2 = PathBuf::from(format!("{}-wal", temp_db_path.display()));
        for wal_path in [&wal_path1, &wal_path2] {
            if wal_path.exists() {
                let _ = fs::remove_file(wal_path);
            }
        }

        let shm_path1 = temp_db_path.with_extension("db-shm");
        let shm_path2 = PathBuf::from(format!("{}-shm", temp_db_path.display()));
        for shm_path in [&shm_path1, &shm_path2] {
            if shm_path.exists() {
                let _ = fs::remove_file(shm_path);
            }
        }
    }
}
