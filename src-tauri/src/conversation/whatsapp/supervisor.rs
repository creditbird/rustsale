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
    sessions_dir: std::path::PathBuf,
    events: Arc<ConversationEventPublisher>,
    clients: Arc<RwLock<HashMap<i64, Arc<dyn WhatsAppClient>>>>,
    cancellation_token: CancellationToken,
    service: std::sync::RwLock<Option<Arc<crate::conversation::service::ConversationService>>>,
}

impl WhatsAppSupervisor {
    /// Creates a new `WhatsAppSupervisor`.
    pub fn new(
        db_path: String,
        sessions_dir: std::path::PathBuf,
        events: Arc<ConversationEventPublisher>,
    ) -> Self {
        Self {
            db_path,
            sessions_dir,
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

    /// Returns the sessions directory path.
    pub fn sessions_dir(&self) -> &std::path::Path {
        &self.sessions_dir
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

    /// Starts a persistent WhatsApp bot for an account and wires message & reaction listeners to the normalizer.
    pub async fn start_account_bot(
        &self,
        account_id: i64,
        account_key: &str,
        session_db_filename: &str,
    ) -> Result<Arc<dyn WhatsAppClient>> {
        // Path traversal guard
        if session_db_filename.contains("..")
            || session_db_filename.contains('/')
            || session_db_filename.contains('\\')
        {
            return Err(anyhow::anyhow!(
                "Invalid session DB filename: {}",
                session_db_filename
            ));
        }

        let session_file = self.sessions_dir.join(session_db_filename);
        let store = whatsapp_rust::prelude::SqliteStore::new(&session_file.to_string_lossy())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to open session SQLite store: {}", e))?;

        let db_path_cloned = self.db_path.clone();
        let events_cloned = self.events.clone();
        let account_id_val = account_id;

        let bot = whatsapp_rust::bot::Bot::builder()
            .with_backend(store)
            .on_message(move |ctx| {
                let db_path = db_path_cloned.clone();
                let events = events_cloned.clone();
                async move {
                    if let Ok(mut conn) = rusqlite::Connection::open(&db_path) {
                        // Save sender_alt if present
                        if let Some(ref sender_alt) = ctx.info.source.sender_alt {
                            let _ = super::phone::save_lid_pn_mapping(
                                &conn,
                                &ctx.info.source.sender.to_string(),
                                &sender_alt.to_string(),
                            );
                        }

                        let text = ctx.message.conversation.clone()
                            .or_else(|| ctx.message.extended_text_message.as_option().and_then(|e| e.text.clone()))
                            .or_else(|| ctx.message.image_message.as_option().and_then(|i| i.caption.clone()))
                            .or_else(|| ctx.message.video_message.as_option().and_then(|v| v.caption.clone()))
                            .or_else(|| ctx.message.document_message.as_option().and_then(|d| d.caption.clone()));

                        let mut attachments = Vec::new();
                        if let Some(img) = ctx.message.image_message.as_option() {
                            attachments.push(super::normalizer::RawWhatsAppAttachment {
                                media_type: "image".to_string(),
                                file_name: None,
                                mime_type: img.mimetype.clone(),
                                size_bytes: img.file_length,
                                url: img.url.clone(),
                                direct_path: img.direct_path.clone(),
                                thumbnail_url: None,
                            });
                        } else if let Some(vid) = ctx.message.video_message.as_option() {
                            attachments.push(super::normalizer::RawWhatsAppAttachment {
                                media_type: "video".to_string(),
                                file_name: None,
                                mime_type: vid.mimetype.clone(),
                                size_bytes: vid.file_length,
                                url: vid.url.clone(),
                                direct_path: vid.direct_path.clone(),
                                thumbnail_url: None,
                            });
                        } else if let Some(aud) = ctx.message.audio_message.as_option() {
                            attachments.push(super::normalizer::RawWhatsAppAttachment {
                                media_type: "audio".to_string(),
                                file_name: None,
                                mime_type: aud.mimetype.clone(),
                                size_bytes: aud.file_length,
                                url: aud.url.clone(),
                                direct_path: aud.direct_path.clone(),
                                thumbnail_url: None,
                            });
                        } else if let Some(doc) = ctx.message.document_message.as_option() {
                            attachments.push(super::normalizer::RawWhatsAppAttachment {
                                media_type: "document".to_string(),
                                file_name: doc.file_name.clone().or_else(|| doc.title.clone()),
                                mime_type: doc.mimetype.clone(),
                                size_bytes: doc.file_length,
                                url: doc.url.clone(),
                                direct_path: doc.direct_path.clone(),
                                thumbnail_url: None,
                            });
                        }

                        let quote = ctx.message.extended_text_message.as_option().and_then(|ext| {
                            ext.context_info.as_option().and_then(|ctx_info| {
                                ctx_info.stanza_id.as_deref().map(|stanza_id| {
                                    super::normalizer::RawWhatsAppQuote {
                                        stanza_id: stanza_id.to_string(),
                                        participant_jid: ctx_info.participant.clone().unwrap_or_default(),
                                        text: ctx_info.quoted_message.as_option().and_then(|qm| qm.conversation.clone()),
                                        media_type: None,
                                        timestamp: None,
                                    }
                                })
                            })
                        });

                        let raw_msg = super::normalizer::RawWhatsAppMessage {
                            id: ctx.info.id.clone(),
                            chat_jid: ctx.info.source.chat.to_string(),
                            sender_jid: Some(ctx.info.source.sender.to_string()),
                            sender_name: if ctx.info.push_name.is_empty() {
                                None
                            } else {
                                Some(ctx.info.push_name.clone())
                            },
                            text,
                            timestamp: Some(ctx.info.timestamp.to_rfc3339()),
                            is_from_me: ctx.info.source.is_from_me,
                            attachments,
                            quote,
                            raw_json: None,
                        };

                        if let Ok(Some(conv_id)) = super::normalizer::WhatsAppNormalizer::ingest_incoming_message(
                            &mut conn,
                            account_id_val,
                            &raw_msg,
                        ) {
                            events.publish(crate::conversation::events::ConversationInvalidatedEvent {
                                conversation_id: conv_id,
                                lead_id: None,
                                channel: crate::conversation::types::Channel::WhatsApp,
                                reason: crate::conversation::events::ConversationInvalidationReason::Message,
                                message_id: None,
                            });
                        }
                    }
                }
            })
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to build WhatsApp bot: {}", e))?;

        let bot_handle = bot.spawn();
        let client_arc = bot_handle.client();
        let real_client: Arc<dyn WhatsAppClient> = Arc::new(
            super::client::RealWhatsAppClient::new(client_arc, Some(bot_handle), account_key),
        );

        self.register_client(account_id, real_client.clone()).await;
        Ok(real_client)
    }

    /// Loads and connects all active, enabled WhatsApp sessions on application startup.
    pub async fn load_active_sessions_on_startup(&self) -> Result<usize> {
        let accounts: Vec<(i64, String, String)> = {
            let conn = rusqlite::Connection::open(&self.db_path)?;
            let mut stmt = conn.prepare(
                "SELECT ca.id, ca.provider_account_key, ws.session_db_filename
                 FROM channel_accounts ca
                 JOIN whatsapp_sessions ws ON ws.channel_account_id = ca.id
                 WHERE ca.channel = 'whatsapp'
                   AND ca.enabled = 1
                   AND ws.is_valid = 1",
            )?;

            let mapped = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            let list = mapped.collect::<rusqlite::Result<Vec<_>>>()?;
            list
        };

        let mut loaded = 0;
        for (acc_id, acc_key, db_filename) in accounts {
            match self.start_account_bot(acc_id, &acc_key, &db_filename).await {
                Ok(_) => {
                    info!(target: "whatsapp_supervisor", acc_id, %acc_key, "Started WhatsApp account bot on startup");
                    loaded += 1;
                }
                Err(e) => {
                    warn!(target: "whatsapp_supervisor", acc_id, %acc_key, %e, "Failed to start WhatsApp account bot on startup");
                }
            }
        }

        // Also recover any interrupted campaigns on startup
        if let Ok(conn) = rusqlite::Connection::open(&self.db_path) {
            let _ = super::campaign::recover_interrupted_campaigns_all(&conn);
        }

        Ok(loaded)
    }

    /// Disconnects an account, terminates its active bot worker and sets status to disconnected.
    pub async fn disconnect_account(&self, account_id: i64) -> Result<()> {
        if let Some(client) = self.get_client(account_id).await {
            let _ = client.disconnect().await;
            self.unregister_client(account_id).await;
        }

        let conn = rusqlite::Connection::open(&self.db_path)?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE channel_accounts
             SET connection_status = 'disconnected', enabled = 0, updated_at = ?1
             WHERE id = ?2 AND channel = 'whatsapp'",
            rusqlite::params![now, account_id],
        )?;

        Ok(())
    }
}
