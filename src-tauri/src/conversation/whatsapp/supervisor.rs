use anyhow::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use super::client::WhatsAppClient;
use crate::conversation::events::ConversationEventPublisher;

/// Represents a connected and enabled WhatsApp account in the local CRM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectedWhatsAppAccount {
    pub id: i64,
    pub provider_account_key: String,
    pub display_name: Option<String>,
    pub phone_number: Option<String>,
}

/// Supervisor responsible for managing WhatsApp client handles, periodic background
/// outbox draining, connection health checks, and lifecycle events.
pub struct WhatsAppSupervisor {
    db_path: String,
    events: Arc<ConversationEventPublisher>,
    clients: Arc<RwLock<HashMap<i64, Arc<dyn WhatsAppClient>>>>,
    cancellation_token: CancellationToken,
    service: std::sync::RwLock<Option<Arc<crate::conversation::service::ConversationService>>>,
}

impl WhatsAppSupervisor {
    /// Creates a new `WhatsAppSupervisor`.
    pub fn new(db_path: String, events: Arc<ConversationEventPublisher>) -> Self {
        Self {
            db_path,
            events,
            clients: Arc::new(RwLock::new(HashMap::new())),
            cancellation_token: CancellationToken::new(),
            service: std::sync::RwLock::new(None),
        }
    }

    /// Returns the database path.
    pub fn db_path(&self) -> &str {
        &self.db_path
    }

    /// Attaches the core `ConversationService` for outbox draining.
    pub fn with_service(&self, service: Arc<crate::conversation::service::ConversationService>) {
        if let Ok(mut guard) = self.service.write() {
            *guard = Some(service);
        }
    }

    /// Returns a reference to the event publisher.
    pub fn events(&self) -> &Arc<ConversationEventPublisher> {
        &self.events
    }

    /// Returns the active cancellation token.
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    /// Signals the supervisor and background loops to shut down.
    pub fn stop(&self) {
        self.cancellation_token.cancel();
    }

    /// Registers an active client instance for a given channel account.
    pub async fn register_client(&self, account_id: i64, client: Arc<dyn WhatsAppClient>) {
        let mut guard = self.clients.write().await;
        guard.insert(account_id, client);
    }

    /// Unregisters and removes a client handle for a given channel account.
    pub async fn unregister_client(&self, account_id: i64) {
        let mut guard = self.clients.write().await;
        guard.remove(&account_id);
    }

    /// Gets a cloned reference to the active client for an account, if registered.
    pub async fn get_client(&self, account_id: i64) -> Option<Arc<dyn WhatsAppClient>> {
        let guard = self.clients.read().await;
        guard.get(&account_id).cloned()
    }

    /// Queries the SQLite database for all enabled WhatsApp accounts with active/connected status.
    pub fn get_connected_accounts(
        &self,
        conn: &Connection,
    ) -> Result<Vec<ConnectedWhatsAppAccount>> {
        let mut stmt = conn.prepare(
            "SELECT ca.id, ca.provider_account_key, ca.display_name, ws.phone_number
             FROM channel_accounts ca
             LEFT JOIN whatsapp_sessions ws ON ws.channel_account_id = ca.id
             WHERE ca.channel = 'whatsapp' 
               AND ca.enabled = 1 
               AND (ca.connection_status = 'connected' OR ca.connection_status = 'synced')
             ORDER BY ca.id ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ConnectedWhatsAppAccount {
                id: row.get(0)?,
                provider_account_key: row.get(1)?,
                display_name: row.get(2)?,
                phone_number: row.get(3)?,
            })
        })?;

        let mut accounts = Vec::new();
        for r in rows {
            accounts.push(r?);
        }
        Ok(accounts)
    }

    /// Internal synchronization & outbox drain for a single WhatsApp account.
    pub async fn sync_account_internal(&self, account: &ConnectedWhatsAppAccount) -> Result<usize> {
        let client_opt = self.get_client(account.id).await;
        let client = match client_opt {
            Some(c) => c,
            None => {
                // No active client registered in-memory for this account
                return Ok(0);
            }
        };

        if !client.is_connected().await {
            warn!(
                target: "whatsapp_supervisor",
                "Client for WhatsApp account {} ({}) is disconnected",
                account.id, account.provider_account_key
            );
            return Ok(0);
        }

        let maybe_service = self.service.read().ok().and_then(|g| g.as_ref().cloned());
        let mut processed_count = 0;

        if let Some(service) = maybe_service {
            loop {
                if self.cancellation_token.is_cancelled() {
                    break;
                }

                match service
                    .process_next_whatsapp_outbox_job(account.id, client.as_ref())
                    .await
                {
                    Ok(true) => {
                        processed_count += 1;
                    }
                    Ok(false) => {
                        // Outbox queue drained
                        break;
                    }
                    Err(e) => {
                        error!(
                            target: "whatsapp_supervisor",
                            "Error processing WhatsApp outbox job for account {}: {}",
                            account.id, e
                        );
                        break;
                    }
                }
            }
        }

        Ok(processed_count)
    }

    /// Background event & polling loop that periodically syncs connected accounts.
    pub async fn run_loop(&self, poll_interval: Duration) {
        info!(target: "whatsapp_supervisor", "WhatsApp supervisor run loop started");

        while !self.cancellation_token.is_cancelled() {
            tokio::select! {
                _ = self.cancellation_token.cancelled() => {
                    info!(target: "whatsapp_supervisor", "WhatsApp supervisor loop cancelled");
                    break;
                }
                _ = tokio::time::sleep(poll_interval) => {
                    let conn_res = Connection::open(&self.db_path);
                    let accounts = match conn_res {
                        Ok(conn) => self.get_connected_accounts(&conn).unwrap_or_default(),
                        Err(e) => {
                            warn!(target: "whatsapp_supervisor", "Failed to open DB for polling: {}", e);
                            Vec::new()
                        }
                    };

                    for account in accounts {
                        if self.cancellation_token.is_cancelled() {
                            break;
                        }
                        if let Err(e) = self.sync_account_internal(&account).await {
                            warn!(
                                target: "whatsapp_supervisor",
                                "Failed to sync WhatsApp account {}: {}",
                                account.id, e
                            );
                        }
                    }
                }
            }
        }

        info!(target: "whatsapp_supervisor", "WhatsApp supervisor run loop stopped");
    }
}
