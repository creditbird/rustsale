use anyhow::{anyhow, Result};
use rusqlite::{Connection, OptionalExtension};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use super::client::FacebookGraphClient;
use super::normalizer::FacebookMessengerNormalizer;
use crate::conversation::events::{
    ConversationEventPublisher, ConversationInvalidatedEvent, ConversationInvalidationReason,
};
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
    cancellation_token: CancellationToken,
    service: std::sync::RwLock<Option<Arc<crate::conversation::service::ConversationService>>>,
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
            cancellation_token: CancellationToken::new(),
            service: std::sync::RwLock::new(None),
        }
    }

    pub fn with_service(&self, service: Arc<crate::conversation::service::ConversationService>) {
        if let Ok(mut guard) = self.service.write() {
            *guard = Some(service);
        }
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    pub fn stop(&self) {
        self.cancellation_token.cancel();
    }

    pub fn get_connected_accounts(
        &self,
        conn: &Connection,
    ) -> Result<Vec<ConnectedMessengerAccount>> {
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

        let page_token = match FacebookTokenVault::get_valid_page_token(
            &conn,
            account.id,
            &account.page_id,
        ) {
            Ok(token) => token,
            Err(e) => {
                let err_msg = format!(
                    "Token resolution failed for page {}: {}",
                    account.page_id, e
                );
                warn!(target: "facebook_sync", "{}", err_msg);
                let _ = conn.execute(
                    "UPDATE channel_accounts SET connection_status = 'invalid', last_error = ?1, updated_at = datetime('now') WHERE id = ?2",
                    rusqlite::params![err_msg, account.id],
                );
                return Err(anyhow!(err_msg));
            }
        };

        // Drain pending outbox jobs for this Messenger account
        let maybe_service = self.service.read().ok().and_then(|g| g.as_ref().cloned());
        if let Some(service) = maybe_service {
            while let Ok(true) = service
                .process_next_messenger_outbox_job(
                    account.id,
                    self.client.as_ref(),
                    &account.page_id,
                    &page_token,
                )
                .await
            {}
        }

        let mut all_new_messages = 0;
        let mut after_cursor: Option<String> = None;
        let max_pages = 25;
        let mut page_count = 0;
        let mut all_affected_conversations: Vec<i64> = Vec::new();

        loop {
            page_count += 1;
            let conversations_page = match self
                .client
                .fetch_page_conversations(
                    &account.page_id,
                    &page_token,
                    50,
                    after_cursor.as_deref(),
                )
                .await
            {
                Ok(p) => p,
                Err(e) => {
                    let err_msg = format!(
                        "Failed fetching conversations for page {}: {}",
                        account.page_id, e
                    );
                    warn!(target: "facebook_sync", "{}", err_msg);
                    let is_auth_failure =
                        if let Some(meta_err) = e.downcast_ref::<super::error::MetaGraphError>() {
                            meta_err.is_auth_failure()
                        } else {
                            let msg = e.to_string();
                            msg.contains("code 190")
                                || msg.contains("code 102")
                                || msg.contains("status 401")
                                || msg.contains("Error validating access token")
                                || msg.contains("Session has expired")
                                || msg.contains("The access token could not be decrypted")
                        };

                    if is_auth_failure {
                        if let Ok(tx) = conn.transaction() {
                            let _ = tx.execute(
                                "UPDATE channel_accounts SET connection_status = 'invalid', last_error = ?1, updated_at = datetime('now') WHERE id = ?2",
                                rusqlite::params![err_msg, account.id],
                            );
                            let _ = tx.execute(
                                "UPDATE facebook_page_tokens SET token_status = 'expired', updated_at = datetime('now') WHERE channel_account_id = ?1",
                                rusqlite::params![account.id],
                            );
                            let _ = tx.commit();
                        }
                    } else {
                        let _ = conn.execute(
                            "UPDATE channel_accounts SET last_error = ?1, updated_at = datetime('now') WHERE id = ?2",
                            rusqlite::params![err_msg, account.id],
                        );
                    }
                    return Err(anyhow!(err_msg));
                }
            };

            let has_threads = !conversations_page.data.is_empty();
            if has_threads {
                let summary = FacebookMessengerNormalizer::ingest_conversations_batch(
                    &mut conn,
                    account.id,
                    &account.page_id,
                    &conversations_page.data,
                )?;
                all_new_messages += summary.new_messages_count;
                for cid in summary.affected_conversation_ids {
                    if !all_affected_conversations.contains(&cid) {
                        all_affected_conversations.push(cid);
                    }
                }
                for delta in summary.delta_events {
                    self.events.publish_delta(delta);
                }
            }

            let next_cursor = conversations_page
                .paging
                .and_then(|p| p.cursors)
                .and_then(|c| c.after)
                .filter(|s| !s.trim().is_empty());

            match next_cursor {
                Some(cursor) if has_threads && page_count < max_pages => {
                    after_cursor = Some(cursor);
                }
                _ => break,
            }
        }

        // Clear error and mark last_connected_at
        let _ = conn.execute(
            "UPDATE channel_accounts SET last_error = NULL, last_connected_at = datetime('now'), updated_at = datetime('now') WHERE id = ?1",
            rusqlite::params![account.id],
        );

        if all_new_messages > 0 && !all_affected_conversations.is_empty() {
            for cid in &all_affected_conversations {
                let lead_id_opt: Option<i64> = conn
                    .query_row(
                        "SELECT ci.lead_id FROM channel_identities ci
                         JOIN conversation_participants cp ON cp.identity_id = ci.id
                         WHERE cp.conversation_id = ?1 AND ci.lead_id IS NOT NULL LIMIT 1",
                        [cid],
                        |r| r.get(0),
                    )
                    .optional()
                    .unwrap_or(None);

                let msg_id_opt: Option<i64> = conn
                    .query_row(
                        "SELECT id FROM messages WHERE conversation_id = ?1 ORDER BY id DESC LIMIT 1",
                        [cid],
                        |r| r.get(0),
                    )
                    .optional()
                    .unwrap_or(None);

                self.events.publish(ConversationInvalidatedEvent {
                    conversation_id: *cid,
                    lead_id: lead_id_opt,
                    channel: Channel::Messenger,
                    reason: ConversationInvalidationReason::Message,
                    message_id: msg_id_opt,
                });
            }
        }

        Ok(all_new_messages)
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

        let account = conn
            .query_row(
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
            )
            .map_err(|e| {
                anyhow!(
                    "No channel account found for Facebook page {}: {}",
                    page_id,
                    e
                )
            })?;

        self.sync_account_internal(&account).await
    }

    pub fn ingest_webhook_events(
        &self,
        page_id: &str,
        events: &[serde_json::Value],
    ) -> Result<usize> {
        let mut conn = Connection::open(&self.db_path)
            .map_err(|e| anyhow!("Failed to open DB connection: {}", e))?;

        let account_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM channel_accounts WHERE channel = 'messenger' AND provider_account_key = ?1 AND enabled = 1",
                rusqlite::params![page_id],
                |row| row.get(0),
            )
            .optional()?;

        let channel_account_id = match account_id {
            Some(id) => id,
            None => {
                warn!(target: "facebook_sync", page_id = %page_id, "Rejecting webhook ingestion: No enabled connected channel account found for Facebook page");
                return Ok(0);
            }
        };

        let mut affected = Vec::new();
        for event in events {
            match FacebookMessengerNormalizer::ingest_webhook_event(
                &mut conn,
                channel_account_id,
                page_id,
                event,
            ) {
                Ok(Some(res)) => {
                    if !affected.contains(&res.conversation_id) {
                        affected.push(res.conversation_id);
                    }

                    self.events.publish(ConversationInvalidatedEvent {
                        conversation_id: res.conversation_id,
                        lead_id: res.lead_id,
                        channel: Channel::Messenger,
                        reason: ConversationInvalidationReason::Message,
                        message_id: res.delta_events.last().map(|d| d.message.id),
                    });

                    for delta_event in res.delta_events {
                        self.events.publish_delta(delta_event);
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    warn!(target: "facebook_sync", %err, "Failed ingesting webhook event");
                }
            }
        }

        Ok(affected.len())
    }

    pub fn start_background_loop(
        self: Arc<Self>,
        poll_interval: Duration,
    ) -> tauri::async_runtime::JoinHandle<()> {
        let cancel = self.cancellation_token.clone();
        tauri::async_runtime::spawn(async move {
            info!(target: "facebook_sync", "Facebook Messenger sync supervisor background loop started");
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        info!(target: "facebook_sync", "Facebook Messenger supervisor background loop received shutdown signal");
                        break;
                    }
                    _ = tokio::time::sleep(poll_interval) => {
                        if let Err(e) = self.sync_all().await {
                            warn!(target: "facebook_sync", error = %e, "Messenger background sync cycle failed");
                        }
                    }
                }
            }
        })
    }
}
