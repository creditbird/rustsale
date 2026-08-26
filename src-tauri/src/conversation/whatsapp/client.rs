use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::normalizer::RawWhatsAppQuote;

/// Response returned after successfully sending a message to WhatsApp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhatsAppSendResponse {
    pub provider_message_id: String,
    pub timestamp: Option<String>,
}

/// Error classification for WhatsApp outbox processing and network interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WhatsAppErrorKind {
    /// Authentication or session expired / revoked (e.g. 401, logged out).
    AuthFailure,
    /// Rate limit or 429 backoff encountered.
    RateLimited,
    /// Destination number or JID is not registered on WhatsApp.
    InvalidRecipient,
    /// Post-dispatch timeout where message state is uncertain.
    AmbiguousTimeout,
    /// Network transient failure (e.g. connection reset, transient timeout).
    NetworkTransient,
    /// Generic non-retryable error.
    Permanent,
}

/// Structured WhatsApp domain error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppError {
    pub kind: WhatsAppErrorKind,
    pub message: String,
    pub retry_after_secs: Option<u64>,
}

impl std::fmt::Display for WhatsAppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for WhatsAppError {}

/// Categorizes any error into `WhatsAppErrorKind`.
pub fn categorize_whatsapp_error(err: &(dyn std::error::Error + 'static)) -> WhatsAppErrorKind {
    if let Some(wa_err) = err.downcast_ref::<WhatsAppError>() {
        return wa_err.kind;
    }
    let msg = err.to_string().to_lowercase();
    if msg.contains("401")
        || msg.contains("unauthorized")
        || msg.contains("session revoked")
        || msg.contains("logged out")
        || msg.contains("invalid session")
        || msg.contains("auth fail")
    {
        WhatsAppErrorKind::AuthFailure
    } else if msg.contains("429") || msg.contains("rate limit") || msg.contains("too many requests")
    {
        WhatsAppErrorKind::RateLimited
    } else if msg.contains("ambiguous timeout")
        || msg.contains("delivery unknown")
        || msg.contains("timeout waiting for server ack")
        || msg.contains("ack timeout")
    {
        WhatsAppErrorKind::AmbiguousTimeout
    } else if msg.contains("not on whatsapp")
        || msg.contains("invalid jid")
        || msg.contains("recipient not found")
        || msg.contains("user not registered")
    {
        WhatsAppErrorKind::InvalidRecipient
    } else if msg.contains("connection reset")
        || msg.contains("broken pipe")
        || msg.contains("timeout")
        || msg.contains("network unreachable")
        || msg.contains("temporary failure")
    {
        WhatsAppErrorKind::NetworkTransient
    } else {
        WhatsAppErrorKind::Permanent
    }
}

/// Core interface for WhatsApp client operations.
#[async_trait]
pub trait WhatsAppClient: Send + Sync {
    /// Sends a plain text message, optionally quoting a previous message.
    async fn send_text_message(
        &self,
        recipient_jid: &str,
        text: &str,
        quote: Option<&RawWhatsAppQuote>,
    ) -> Result<WhatsAppSendResponse>;

    /// Sends a media message (image, video, audio, document/file, sticker), optionally quoting a previous message.
    async fn send_media_message(
        &self,
        recipient_jid: &str,
        media_type: &str,
        media_data_or_url: &str,
        caption: Option<&str>,
        quote: Option<&RawWhatsAppQuote>,
    ) -> Result<WhatsAppSendResponse>;

    /// Sends an emoji reaction to a target message.
    async fn send_reaction(
        &self,
        chat_jid: &str,
        target_message_id: &str,
        target_sender_jid: Option<&str>,
        emoji: &str,
    ) -> Result<()>;

    /// Returns true if the client session is actively connected and ready for traffic.
    async fn is_connected(&self) -> bool;
}

/// Recorded payload for sent text message in `MockWhatsAppClient`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockSentTextMessage {
    pub recipient_jid: String,
    pub text: String,
    pub quote: Option<RawWhatsAppQuote>,
}

/// Recorded payload for sent media message in `MockWhatsAppClient`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockSentMediaMessage {
    pub recipient_jid: String,
    pub media_type: String,
    pub media_data_or_url: String,
    pub caption: Option<String>,
    pub quote: Option<RawWhatsAppQuote>,
}

/// Recorded payload for sent reaction in `MockWhatsAppClient`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockSentReaction {
    pub chat_jid: String,
    pub target_message_id: String,
    pub target_sender_jid: Option<String>,
    pub emoji: String,
}

/// Deterministic mock implementation of `WhatsAppClient` for testing.
#[derive(Clone)]
pub struct MockWhatsAppClient {
    pub sent_messages: Arc<Mutex<Vec<MockSentTextMessage>>>,
    pub sent_media: Arc<Mutex<Vec<MockSentMediaMessage>>>,
    pub sent_reactions: Arc<Mutex<Vec<MockSentReaction>>>,
    pub next_send_result: Arc<Mutex<Option<Result<WhatsAppSendResponse>>>>,
    pub next_reaction_result: Arc<Mutex<Option<Result<()>>>>,
    pub connected: Arc<AtomicBool>,
}

impl Default for MockWhatsAppClient {
    fn default() -> Self {
        Self::new()
    }
}

impl MockWhatsAppClient {
    pub fn new() -> Self {
        Self {
            sent_messages: Arc::new(Mutex::new(Vec::new())),
            sent_media: Arc::new(Mutex::new(Vec::new())),
            sent_reactions: Arc::new(Mutex::new(Vec::new())),
            next_send_result: Arc::new(Mutex::new(None)),
            next_reaction_result: Arc::new(Mutex::new(None)),
            connected: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn set_next_send_result(&self, res: Result<WhatsAppSendResponse>) {
        *self.next_send_result.lock().unwrap() = Some(res);
    }

    pub fn set_next_send_error(&self, kind: WhatsAppErrorKind, message: impl Into<String>) {
        let err = WhatsAppError {
            kind,
            message: message.into(),
            retry_after_secs: None,
        };
        *self.next_send_result.lock().unwrap() = Some(Err(anyhow::anyhow!(err)));
    }

    pub fn set_next_reaction_result(&self, res: Result<()>) {
        *self.next_reaction_result.lock().unwrap() = Some(res);
    }

    pub fn set_connected(&self, connected: bool) {
        self.connected.store(connected, Ordering::SeqCst);
    }

    pub fn get_sent_messages(&self) -> Vec<MockSentTextMessage> {
        self.sent_messages.lock().unwrap().clone()
    }

    pub fn get_sent_media(&self) -> Vec<MockSentMediaMessage> {
        self.sent_media.lock().unwrap().clone()
    }

    pub fn get_sent_reactions(&self) -> Vec<MockSentReaction> {
        self.sent_reactions.lock().unwrap().clone()
    }

    pub fn clear(&self) {
        self.sent_messages.lock().unwrap().clear();
        self.sent_media.lock().unwrap().clear();
        self.sent_reactions.lock().unwrap().clear();
        *self.next_send_result.lock().unwrap() = None;
        *self.next_reaction_result.lock().unwrap() = None;
    }
}

#[async_trait]
impl WhatsAppClient for MockWhatsAppClient {
    async fn send_text_message(
        &self,
        recipient_jid: &str,
        text: &str,
        quote: Option<&RawWhatsAppQuote>,
    ) -> Result<WhatsAppSendResponse> {
        self.sent_messages
            .lock()
            .unwrap()
            .push(MockSentTextMessage {
                recipient_jid: recipient_jid.to_string(),
                text: text.to_string(),
                quote: quote.cloned(),
            });

        if let Some(res) = self.next_send_result.lock().unwrap().take() {
            res
        } else {
            Ok(WhatsAppSendResponse {
                provider_message_id: format!("wa_msg_{:016x}", rand::random::<u64>()),
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
            })
        }
    }

    async fn send_media_message(
        &self,
        recipient_jid: &str,
        media_type: &str,
        media_data_or_url: &str,
        caption: Option<&str>,
        quote: Option<&RawWhatsAppQuote>,
    ) -> Result<WhatsAppSendResponse> {
        self.sent_media.lock().unwrap().push(MockSentMediaMessage {
            recipient_jid: recipient_jid.to_string(),
            media_type: media_type.to_string(),
            media_data_or_url: media_data_or_url.to_string(),
            caption: caption.map(str::to_string),
            quote: quote.cloned(),
        });

        if let Some(res) = self.next_send_result.lock().unwrap().take() {
            res
        } else {
            Ok(WhatsAppSendResponse {
                provider_message_id: format!("wa_media_{:016x}", rand::random::<u64>()),
                timestamp: Some(chrono::Utc::now().to_rfc3339()),
            })
        }
    }

    async fn send_reaction(
        &self,
        chat_jid: &str,
        target_message_id: &str,
        target_sender_jid: Option<&str>,
        emoji: &str,
    ) -> Result<()> {
        self.sent_reactions.lock().unwrap().push(MockSentReaction {
            chat_jid: chat_jid.to_string(),
            target_message_id: target_message_id.to_string(),
            target_sender_jid: target_sender_jid.map(str::to_string),
            emoji: emoji.to_string(),
        });

        if let Some(res) = self.next_reaction_result.lock().unwrap().take() {
            res
        } else {
            Ok(())
        }
    }

    async fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }
}
