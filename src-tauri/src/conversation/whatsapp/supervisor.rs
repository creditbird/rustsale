use anyhow::Result;
use rusqlite::{Connection, OptionalExtension};
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
    outbox_notify: Arc<tokio::sync::Notify>,
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
            outbox_notify: Arc::new(tokio::sync::Notify::new()),
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

    /// Notifies the supervisor to immediately drain outbox jobs for connected accounts.
    pub fn notify_outbox(&self) {
        self.outbox_notify.notify_waiters();
    }

    /// Drains outbox jobs for all currently connected and enabled WhatsApp accounts.
    pub async fn drain_all_connected_accounts(&self) {
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

    /// Background event & polling loop that periodically syncs connected accounts and reacts to outbox notifications.
    pub async fn run_loop(&self, poll_interval: Duration) {
        info!(target: "whatsapp_supervisor", "WhatsApp supervisor run loop started");

        let effective_interval = poll_interval.min(Duration::from_millis(200));
        let mut ticker = tokio::time::interval(effective_interval);

        while !self.cancellation_token.is_cancelled() {
            tokio::select! {
                _ = self.cancellation_token.cancelled() => {
                    info!(target: "whatsapp_supervisor", "WhatsApp supervisor loop cancelled");
                    break;
                }
                _ = ticker.tick() => {
                    self.drain_all_connected_accounts().await;
                }
                _ = self.outbox_notify.notified() => {
                    self.drain_all_connected_accounts().await;
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

        // Clean up any existing client before starting a new one
        if let Some(existing_client) = self.get_client(account_id).await {
            let _ = existing_client.disconnect().await;
            self.unregister_client(account_id).await;
        }

        let session_file = self.sessions_dir.join(session_db_filename);
        let store = whatsapp_rust::prelude::SqliteStore::new(&session_file.to_string_lossy())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to open session SQLite store: {}", e))?;

        let db_path_cloned = self.db_path.clone();
        let db_path_logged_out = self.db_path.clone();
        let events_cloned = self.events.clone();
        let account_id_val = account_id;
        let account_id_logged_out = account_id;

        let bot = whatsapp_rust::bot::Bot::builder()
            .with_backend(store)
            .on_logged_out(move |info| {
                let db_path = db_path_logged_out.clone();
                async move {
                    warn!(
                        target: "whatsapp_supervisor",
                        account_id = account_id_logged_out,
                        reason = ?info.reason,
                        "WhatsApp session logged out from primary device"
                    );
                    if let Ok(conn) = rusqlite::Connection::open(&db_path) {
                        let _ = conn.execute(
                            "UPDATE channel_accounts SET connection_status = 'disconnected', last_error = 'Session unlinked from phone' WHERE id = ?1",
                            [account_id_logged_out],
                        );
                        let _ = conn.execute(
                            "UPDATE whatsapp_sessions SET is_valid = 0, last_error = 'Session unlinked from phone' WHERE channel_account_id = ?1",
                            [account_id_logged_out],
                        );
                    }
                }
            })
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

                        let chat_jid = ctx.info.source.chat.to_string();
                        let is_from_me = ctx.info.source.is_from_me;
                        let sender_jid = if is_from_me {
                            None
                        } else {
                            Some(ctx.info.source.sender.to_string())
                        };

                        let key = whatsapp_rust::prelude::wa::MessageKey {
                            id: Some(ctx.info.id.clone()),
                            remote_jid: Some(chat_jid.clone()),
                            from_me: Some(is_from_me),
                            participant: sender_jid,
                            ..Default::default()
                        };

                        let timestamp_secs = Some(ctx.info.timestamp.timestamp() as u64);
                        let push_name = if ctx.info.push_name.is_empty() {
                            None
                        } else {
                            Some(ctx.info.push_name.as_str())
                        };

                        if let Some(reaction) = super::normalizer::WhatsAppNormalizer::extract_raw_reaction(
                            &ctx.message,
                            &key,
                            timestamp_secs,
                            &chat_jid,
                        ) {
                            if let Ok(()) = super::normalizer::WhatsAppNormalizer::ingest_reaction(
                                &mut conn,
                                account_id_val,
                                &reaction,
                            ) {
                                let conv_id_opt: Option<i64> = conn.query_row(
                                    "SELECT id FROM channel_conversations WHERE channel_account_id = ?1 AND (external_thread_id = ?2 OR external_thread_id = ?3)",
                                    rusqlite::params![
                                        account_id_val,
                                        reaction.chat_jid,
                                        super::phone::phone_to_direct_jid(&super::phone::jid_to_phone(&reaction.chat_jid).unwrap_or_default())
                                    ],
                                    |r| r.get(0),
                                ).optional().unwrap_or(None);

                                if let Some(conv_id) = conv_id_opt {
                                    let lead_id_opt: Option<i64> = conn.query_row(
                                        "SELECT ci.lead_id FROM channel_identities ci
                                         JOIN conversation_participants cp ON cp.identity_id = ci.id
                                         WHERE cp.conversation_id = ?1 AND ci.lead_id IS NOT NULL LIMIT 1",
                                        [conv_id],
                                        |r| r.get(0),
                                    ).optional().unwrap_or(None);

                                    events.publish(crate::conversation::events::ConversationInvalidatedEvent {
                                        conversation_id: conv_id,
                                        lead_id: lead_id_opt,
                                        channel: crate::conversation::types::Channel::WhatsApp,
                                        reason: crate::conversation::events::ConversationInvalidationReason::Reaction,
                                        message_id: None,
                                    });
                                }
                            }
                        } else if let Some(raw_msg) = super::normalizer::WhatsAppNormalizer::extract_raw_message(
                            &ctx.message,
                            &key,
                            timestamp_secs,
                            push_name,
                            &chat_jid,
                            None,
                        ) {
                            if let Ok(Some(conv_id)) = super::normalizer::WhatsAppNormalizer::ingest_incoming_message(
                                &mut conn,
                                account_id_val,
                                &raw_msg,
                            ) {
                                let lead_id_opt: Option<i64> = conn.query_row(
                                    "SELECT ci.lead_id FROM channel_identities ci
                                     JOIN conversation_participants cp ON cp.identity_id = ci.id
                                     WHERE cp.conversation_id = ?1 AND ci.lead_id IS NOT NULL LIMIT 1",
                                    [conv_id],
                                    |r| r.get(0),
                                ).optional().unwrap_or(None);

                                let msg_id_opt: Option<i64> = conn.query_row(
                                    "SELECT id FROM messages WHERE conversation_id = ?1 AND provider_message_id = ?2",
                                    rusqlite::params![conv_id, raw_msg.id],
                                    |r| r.get(0),
                                ).optional().unwrap_or(None);

                                events.publish(crate::conversation::events::ConversationInvalidatedEvent {
                                    conversation_id: conv_id,
                                    lead_id: lead_id_opt,
                                    channel: crate::conversation::types::Channel::WhatsApp,
                                    reason: crate::conversation::events::ConversationInvalidationReason::Message,
                                    message_id: msg_id_opt,
                                });
                                // Keep the protobuf thumbnail from ingestion. The full encrypted
                                // payload is resolved lazily through `download_conversation_attachment`,
                                // which owns size limits, media-retry, persistence, and error semantics.
                            }
                        }
                    }
                }
            })
            .on_event({
                let db_path = self.db_path.clone();
                let events = self.events.clone();
                move |event, _client| {
                    let db_path = db_path.clone();
                    let events = events.clone();
                    async move {
                        if let whatsapp_rust::types::events::Event::HistorySync(lazy_hs) = event.as_ref() {
                            if let Some(history_sync) = lazy_hs.get() {
                                if let Ok(mut conn) = rusqlite::Connection::open(&db_path) {
                                    // Save phoneNumberToLidMappings from sync
                                    for mapping in &history_sync.phone_number_to_lid_mappings {
                                        if let (Some(ref pn_jid), Some(ref lid_jid)) = (mapping.pn_jid.as_ref(), mapping.lid_jid.as_ref()) {
                                            if let Some(pn) = super::phone::jid_to_phone(pn_jid) {
                                                let _ = super::phone::save_lid_pn_mapping(&conn, lid_jid, &pn);
                                            }
                                        }
                                    }

                                    let mut invalidated_conv_ids = std::collections::HashSet::new();

                                    for conv in &history_sync.conversations {
                                        let chat_jid = conv.id.clone();
                                        let conv_title = conv.name.as_deref().or(conv.display_name.as_deref());

                                        if let (Some(ref pn_jid), Some(ref lid_jid)) = (conv.pn_jid.as_ref(), conv.lid_jid.as_ref()) {
                                            if let Some(pn) = super::phone::jid_to_phone(pn_jid) {
                                                let _ = super::phone::save_lid_pn_mapping(&conn, lid_jid, &pn);
                                            }
                                        }

                                        // Ensure conversation & identity exists in DB
                                        if let Ok(conv_id) = super::normalizer::WhatsAppNormalizer::ensure_conversation_exists(
                                            &mut conn,
                                            account_id_val,
                                            &chat_jid,
                                            conv_title,
                                        ) {
                                            invalidated_conv_ids.insert(conv_id);
                                        }

                                        // Ingest historical messages & reactions
                                        for history_msg in &conv.messages {
                                            if let Some(web_msg) = history_msg.message.as_option() {
                                                if let Some(ref msg_content) = web_msg.message.as_option() {
                                                    if let Some(ref key) = web_msg.key.as_option() {
                                                        let push_name = web_msg.push_name.as_deref();
                                                        if let Some(reaction) = super::normalizer::WhatsAppNormalizer::extract_raw_reaction(
                                                            msg_content,
                                                            key,
                                                            web_msg.message_timestamp,
                                                            &chat_jid,
                                                        ) {
                                                            let _ = super::normalizer::WhatsAppNormalizer::ingest_reaction(
                                                                &mut conn,
                                                                account_id_val,
                                                                &reaction,
                                                            );
                                                        } else if let Some(raw_msg) = super::normalizer::WhatsAppNormalizer::extract_raw_message(
                                                            msg_content,
                                                            key,
                                                            web_msg.message_timestamp,
                                                            push_name,
                                                            &chat_jid,
                                                            conv_title,
                                                        ) {
                                                            if let Ok(Some(conv_id)) = super::normalizer::WhatsAppNormalizer::ingest_incoming_message(
                                                                &mut conn,
                                                                account_id_val,
                                                                &raw_msg,
                                                            ) {
                                                                invalidated_conv_ids.insert(conv_id);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                     for conv_id in invalidated_conv_ids {
                                        let lead_id_opt: Option<i64> = conn.query_row(
                                            "SELECT ci.lead_id FROM channel_identities ci
                                             JOIN conversation_participants cp ON cp.identity_id = ci.id
                                             WHERE cp.conversation_id = ?1 AND ci.lead_id IS NOT NULL LIMIT 1",
                                            [conv_id],
                                            |r| r.get(0),
                                        ).optional().unwrap_or(None);

                                        events.publish(crate::conversation::events::ConversationInvalidatedEvent {
                                            conversation_id: conv_id,
                                            lead_id: lead_id_opt,
                                            channel: crate::conversation::types::Channel::WhatsApp,
                                            reason: crate::conversation::events::ConversationInvalidationReason::Message,
                                            message_id: None,
                                        });
                                    }
                                }
                            }
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
        // Upgrade empty capabilities for existing WhatsApp accounts if needed
        if let Ok(conn) = rusqlite::Connection::open(&self.db_path) {
            let wa_caps = serde_json::to_string(
                &crate::conversation::types::ConversationCapabilities::whatsapp(),
            )
            .unwrap_or_else(|_| "{}".to_string());
            let _ = conn.execute(
                "UPDATE channel_accounts SET capabilities_json = ?1 WHERE channel = 'whatsapp' AND (capabilities_json IS NULL OR capabilities_json = '' OR capabilities_json = '{}')",
                rusqlite::params![wa_caps],
            );
        }
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
            // If already connected and active, do not recreate or teardown existing connection!
            if let Some(client) = self.get_client(acc_id).await {
                if client.is_connected().await {
                    loaded += 1;
                    continue;
                }
            }

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

    /// Disconnects an account, terminates its active bot worker, cleans up session files and sets status to disconnected.
    pub async fn disconnect_account(&self, account_id: i64) -> Result<()> {
        if let Some(client) = self.get_client(account_id).await {
            let _ = client.disconnect().await;
            self.unregister_client(account_id).await;
        }

        let conn = rusqlite::Connection::open(&self.db_path)?;
        let now = chrono::Utc::now().to_rfc3339();

        let session_file_opt: Option<String> = conn
            .query_row(
                "SELECT session_db_filename FROM whatsapp_sessions WHERE channel_account_id = ?1",
                [account_id],
                |r| r.get(0),
            )
            .ok();

        if let Some(db_name) = session_file_opt {
            let session_path = self.sessions_dir.join(&db_name);
            super::login::WhatsAppLoginManager::clean_temp_db_files(&session_path);
        }

        conn.execute(
            "UPDATE channel_accounts
             SET connection_status = 'disconnected', enabled = 0, updated_at = ?1
             WHERE id = ?2 AND channel = 'whatsapp'",
            rusqlite::params![now, account_id],
        )?;

        conn.execute(
            "UPDATE whatsapp_sessions
             SET is_valid = 0, updated_at = ?1
             WHERE channel_account_id = ?2",
            rusqlite::params![now, account_id],
        )?;

        Ok(())
    }
}
