use std::sync::Arc;
use std::time::Duration;
use anyhow::{anyhow, Result};
use rusqlite::Connection;
use tracing::{error, info, warn};

use super::client::FacebookGraphClient;
use super::normalizer::FacebookMessengerNormalizer;
use crate::conversation::events::{ConversationEventPublisher, ConversationInvalidatedEvent, ConversationInvalidationReason};
use crate::conversation::types::Channel;
use crate::facebook::vault::FacebookTokenVault;

#[derive(Debug, Clone)]
pub struct ConnectedMessengerAccount {
    pub id: i64,
    pub page_id: String,
    pub display_name: Option<String>,
}

pub struct FacebookMessengerSupervisor {
    db_path: String,
    events: Arc<ConversationEventPublisher>,
    client: Arc<dyn FacebookGraphClient>,
}

impl FacebookMessengerSupervisor {
    pub fn new(
        db_path: String,
        events: Arc<ConversationEventPublisher>,
        client: Arc<dyn FacebookGraphClient>,
    ) -> Self {
        Self {
            db_path,
            events,
            client,
        }
    }

    pub fn get_connected_accounts(&self, conn: &Connection) -> Result<Vec<ConnectedMessengerAccount>> {
        let mut stmt = conn.prepare(
            "SELECT id, provider_account_key, display_name
             FROM channel_accounts
             WHERE channel = 'messenger' AND connection_status = 'connected' AND enabled = 1
             ORDER BY id ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ConnectedMessengerAccount {
                id: row.get(0)?,
                page_id: row.get(1)?,
                display_name: row.get(2)?,
            })
        })?;

        let mut accounts = Vec::new();
        for r in rows {
            accounts.push(r?);
        }
        Ok(accounts)
    }

    pub async fn sync_account_internal(
        &self,
        account: &ConnectedMessengerAccount,
    ) -> Result<usize> {
        let mut conn = Connection::open(&self.db_path)
            .map_err(|e| anyhow!("Failed to open DB connection for messenger sync: {}", e))?;

        let page_token = match FacebookTokenVault::get_valid_page_token(&conn, account.id, &account.page_id) {
            Ok(token) => token,
            Err(e) => {
                let err_msg = format!("Token resolution failed for page {}: {}", account.page_id, e);
                warn!(target: "facebook_sync", "{}", err_msg);
                let _ = conn.execute(
                    "UPDATE channel_accounts SET last_error = ?1, updated_at = datetime('now') WHERE id = ?2",
                    rusqlite::params![err_msg, account.id],
                );
                return Err(anyhow!(err_msg));
            }
        };

        let conversations_page = self
            .client
            .fetch_page_conversations(&account.page_id, &page_token, 50)
            .await
            .map_err(|e| anyhow!("Failed fetching conversations from Meta for page {}: {}", account.page_id, e))?;

        let summary = FacebookMessengerNormalizer::ingest_conversations_batch(
            &mut conn,
            account.id,
            &account.page_id,
            &conversations_page.data,
        )?;

        // Clear error and mark last_connected_at
        let _ = conn.execute(
            "UPDATE channel_accounts SET last_error = NULL, last_connected_at = datetime('now'), updated_at = datetime('now') WHERE id = ?1",
            rusqlite::params![account.id],
        );

        if summary.new_messages_count > 0 || summary.conversations_synced > 0 {
            self.events.publish(ConversationInvalidatedEvent {
                conversation_id: 0,
                lead_id: None,
                channel: Channel::Messenger,
                reason: ConversationInvalidationReason::Message,
                message_id: None,
            });
        }

        Ok(summary.new_messages_count)
    }

    pub async fn sync_all(&self) -> Result<usize> {
        let conn = Connection::open(&self.db_path)
            .map_err(|e| anyhow!("Failed to open DB connection: {}", e))?;

        let accounts = self.get_connected_accounts(&conn)?;
        if accounts.is_empty() {
            return Ok(0);
        }

        let mut total_new = 0;
        for acc in accounts {
            match self.sync_account_internal(&acc).await {
                Ok(new_count) => {
                    total_new += new_count;
                }
                Err(e) => {
                    error!(target: "facebook_sync", page_id = %acc.page_id, error = %e, "Failed to sync Facebook Messenger page");
                }
            }
        }
        Ok(total_new)
    }

    pub async fn sync_page(&self, page_id: &str) -> Result<usize> {
        let conn = Connection::open(&self.db_path)
            .map_err(|e| anyhow!("Failed to open DB connection: {}", e))?;

        let account = conn.query_row(
            "SELECT id, provider_account_key, display_name
             FROM channel_accounts
             WHERE channel = 'messenger' AND provider_account_key = ?1",
            rusqlite::params![page_id],
            |row| {
                Ok(ConnectedMessengerAccount {
                    id: row.get(0)?,
                    page_id: row.get(1)?,
                    display_name: row.get(2)?,
                })
            },
        ).map_err(|e| anyhow!("No channel account found for Facebook page {}: {}", page_id, e))?;

        self.sync_account_internal(&account).await
    }

    pub fn start_background_loop(self: Arc<Self>, poll_interval: Duration) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            info!(target: "facebook_sync", "Facebook Messenger sync supervisor background loop started");
            loop {
                if let Err(e) = self.sync_all().await {
                    warn!(target: "facebook_sync", error = %e, "Messenger background sync cycle failed");
                }
                tokio::time::sleep(poll_interval).await;
            }
        })
    }
}
