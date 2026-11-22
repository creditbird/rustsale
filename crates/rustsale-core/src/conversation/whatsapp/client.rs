use anyhow::{Context as _, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use whatsapp_rust::wacore_binary::JidExt;

use super::normalizer::RawWhatsAppQuote;

/// Upload metadata returned after uploading media to WhatsApp CDN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhatsAppMediaUploadMetadata {
    pub url: String,
    pub direct_path: String,
    pub media_key: String,
    pub file_sha256: String,
    pub file_enc_sha256: String,
    pub file_length: u64,
}

/// Response returned after successfully sending a message to WhatsApp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhatsAppSendResponse {
    pub provider_message_id: String,
    pub timestamp: Option<String>,
    #[serde(default)]
    pub media_metadata: Option<WhatsAppMediaUploadMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhatsAppMediaReuploadRequest {
    pub provider_message_id: String,
    pub chat_jid: String,
    pub media_key: Vec<u8>,
    pub is_from_me: bool,
    pub participant_jid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhatsAppMediaReuploadResult {
    Success { direct_path: String },
    NotFound,
    DecryptionError,
    GeneralError,
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
        file_name: Option<&str>,
        mime_type: Option<&str>,
    ) -> Result<WhatsAppSendResponse>;

    /// Sends an emoji reaction to a target message.
    async fn send_reaction(
        &self,
        chat_jid: &str,
        target_message_id: &str,
        target_sender_jid: Option<&str>,
        emoji: &str,
        is_from_me: bool,
    ) -> Result<()>;

    /// Downloads and decrypts a media attachment from WhatsApp CDN.
    async fn download_media(
        &self,
        params: &whatsapp_rust::download::DownloadParams,
    ) -> Result<Vec<u8>>;

    /// Requests a fresh CDN direct path after WhatsApp rejects an expired media reference.
    async fn request_media_reupload(
        &self,
        request: &WhatsAppMediaReuploadRequest,
    ) -> Result<WhatsAppMediaReuploadResult>;

    /// Fetches live group metadata and participant list from WhatsApp.
    async fn get_group_info(&self, group_jid: &str) -> Result<whatsapp_rust::GroupMetadata>;

    /// Returns true if the client session is actively connected and ready for traffic.
    async fn is_connected(&self) -> bool;

    /// Disconnects and shuts down the active connection/bot worker.
    async fn disconnect(&self) -> Result<()>;
}

/// Real production WhatsApp client backed by `whatsapp-rust`.
pub struct RealWhatsAppClient {
    pub client: Arc<whatsapp_rust::Client>,
    pub bot_handle: Arc<tokio::sync::Mutex<Option<whatsapp_rust::bot::BotHandle>>>,
    pub account_key: String,
}

impl RealWhatsAppClient {
    pub fn new(
        client: Arc<whatsapp_rust::Client>,
        bot_handle: Option<whatsapp_rust::bot::BotHandle>,
        account_key: impl Into<String>,
    ) -> Self {
        Self {
            client,
            bot_handle: Arc::new(tokio::sync::Mutex::new(bot_handle)),
            account_key: account_key.into(),
        }
    }
}

#[async_trait]
impl WhatsAppClient for RealWhatsAppClient {
    async fn send_text_message(
        &self,
        recipient_jid: &str,
        text: &str,
        quote: Option<&RawWhatsAppQuote>,
    ) -> Result<WhatsAppSendResponse> {
        let jid: whatsapp_rust::Jid = recipient_jid
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid recipient JID '{}': {}", recipient_jid, e))?;

        let mut msg = whatsapp_rust::prelude::wa::Message {
            conversation: Some(text.to_string()),
            ..Default::default()
        };
        if let Some(q) = quote {
            let context = whatsapp_rust::prelude::wa::ContextInfo {
                stanza_id: Some(q.stanza_id.clone()),
                participant: Some(q.participant_jid.clone()),
                quoted_message: whatsapp_rust::prelude::MessageField::some(
                    whatsapp_rust::prelude::wa::Message {
                        conversation: q.text.clone(),
                        ..Default::default()
                    },
                ),
                ..Default::default()
            };
            msg.extended_text_message = whatsapp_rust::prelude::MessageField::some(
                whatsapp_rust::prelude::wa::message::ExtendedTextMessage {
                    text: Some(text.to_string()),
                    context_info: whatsapp_rust::prelude::MessageField::some(context),
                    ..Default::default()
                },
            );
        }

        let sent = self
            .client
            .send_message(jid, msg)
            .await
            .map_err(|e| anyhow::anyhow!("WhatsApp send error: {}", e))?;

        Ok(WhatsAppSendResponse {
            provider_message_id: sent.message_id,
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            media_metadata: None,
        })
    }

    async fn send_media_message(
        &self,
        recipient_jid: &str,
        media_type: &str,
        media_data_or_url: &str,
        caption: Option<&str>,
        _quote: Option<&RawWhatsAppQuote>,
        file_name: Option<&str>,
        mime_type: Option<&str>,
    ) -> Result<WhatsAppSendResponse> {
        let jid: whatsapp_rust::Jid = recipient_jid
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid recipient JID '{}': {}", recipient_jid, e))?;

        let data = if std::path::Path::new(media_data_or_url).exists() {
            tokio::fs::read(media_data_or_url).await?
        } else if media_data_or_url.starts_with("http://")
            || media_data_or_url.starts_with("https://")
        {
            let resp = reqwest::get(media_data_or_url).await?;
            resp.bytes().await?.to_vec()
        } else if let Some(comma_pos) = media_data_or_url.find(',') {
            if media_data_or_url.starts_with("data:") {
                use base64::Engine as _;
                let b64 = &media_data_or_url[comma_pos + 1..];
                base64::engine::general_purpose::STANDARD
                    .decode(b64.trim())
                    .map_err(|e| anyhow::anyhow!("Failed to decode data URL: {}", e))?
            } else {
                return Err(anyhow::anyhow!(
                    "Invalid media path or url: {}",
                    media_data_or_url
                ));
            }
        } else {
            return Err(anyhow::anyhow!(
                "Invalid media path or url: {}",
                media_data_or_url
            ));
        };

        let wa_media_type = match media_type.to_lowercase().as_str() {
            "image" => whatsapp_rust::wacore::download::MediaType::Image,
            "video" => whatsapp_rust::wacore::download::MediaType::Video,
            "audio" => whatsapp_rust::wacore::download::MediaType::Audio,
            _ => whatsapp_rust::wacore::download::MediaType::Document,
        };

        let data_len = data.len() as u64;
        let upload_res = self
            .client
            .upload(
                data,
                wa_media_type,
                whatsapp_rust::upload::UploadOptions::default(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("Media upload failed: {}", e))?;

        use base64::Engine as _;
        let media_metadata = WhatsAppMediaUploadMetadata {
            url: upload_res.url.clone(),
            direct_path: upload_res.direct_path.clone(),
            media_key: base64::engine::general_purpose::STANDARD.encode(upload_res.media_key),
            file_sha256: base64::engine::general_purpose::STANDARD.encode(upload_res.file_sha256),
            file_enc_sha256: base64::engine::general_purpose::STANDARD
                .encode(upload_res.file_enc_sha256),
            file_length: data_len,
        };

        let mut msg = whatsapp_rust::prelude::wa::Message::default();
        match wa_media_type {
            whatsapp_rust::wacore::download::MediaType::Image => {
                msg.image_message = whatsapp_rust::prelude::MessageField::some(
                    whatsapp_rust::prelude::wa::message::ImageMessage {
                        url: Some(upload_res.url),
                        direct_path: Some(upload_res.direct_path),
                        media_key: Some(upload_res.media_key.to_vec()),
                        file_enc_sha256: Some(upload_res.file_enc_sha256.to_vec()),
                        file_sha256: Some(upload_res.file_sha256.to_vec()),
                        file_length: Some(data_len),
                        caption: caption.map(str::to_string),
                        ..Default::default()
                    },
                );
            }
            whatsapp_rust::wacore::download::MediaType::Video => {
                msg.video_message = whatsapp_rust::prelude::MessageField::some(
                    whatsapp_rust::prelude::wa::message::VideoMessage {
                        url: Some(upload_res.url),
                        direct_path: Some(upload_res.direct_path),
                        media_key: Some(upload_res.media_key.to_vec()),
                        file_enc_sha256: Some(upload_res.file_enc_sha256.to_vec()),
                        file_sha256: Some(upload_res.file_sha256.to_vec()),
                        file_length: Some(data_len),
                        caption: caption.map(str::to_string),
                        ..Default::default()
                    },
                );
            }
            whatsapp_rust::wacore::download::MediaType::Audio => {
                msg.audio_message = whatsapp_rust::prelude::MessageField::some(
                    whatsapp_rust::prelude::wa::message::AudioMessage {
                        url: Some(upload_res.url),
                        direct_path: Some(upload_res.direct_path),
                        media_key: Some(upload_res.media_key.to_vec()),
                        file_enc_sha256: Some(upload_res.file_enc_sha256.to_vec()),
                        file_sha256: Some(upload_res.file_sha256.to_vec()),
                        file_length: Some(data_len),
                        ..Default::default()
                    },
                );
            }
            _ => {
                let resolved_title = file_name.or(caption).map(str::to_string);
                msg.document_message = whatsapp_rust::prelude::MessageField::some(
                    whatsapp_rust::prelude::wa::message::DocumentMessage {
                        url: Some(upload_res.url),
                        direct_path: Some(upload_res.direct_path),
                        media_key: Some(upload_res.media_key.to_vec()),
                        file_enc_sha256: Some(upload_res.file_enc_sha256.to_vec()),
                        file_sha256: Some(upload_res.file_sha256.to_vec()),
                        file_length: Some(data_len),
                        caption: caption.map(str::to_string),
                        title: resolved_title.clone(),
                        file_name: file_name.map(str::to_string).or(resolved_title),
                        mimetype: mime_type.map(str::to_string),
                        ..Default::default()
                    },
                );
            }
        }

        let sent = self
            .client
            .send_message(jid, msg)
            .await
            .map_err(|e| anyhow::anyhow!("WhatsApp media send error: {}", e))?;

        Ok(WhatsAppSendResponse {
            provider_message_id: sent.message_id,
            timestamp: Some(chrono::Utc::now().to_rfc3339()),
            media_metadata: Some(media_metadata),
        })
    }

    async fn send_reaction(
        &self,
        chat_jid: &str,
        target_message_id: &str,
        target_sender_jid: Option<&str>,
        emoji: &str,
        is_from_me: bool,
    ) -> Result<()> {
        let normalized_chat_jid = if chat_jid.contains('@') {
            chat_jid.to_string()
        } else {
            super::phone::phone_to_direct_jid(chat_jid)
        };

        let chat: whatsapp_rust::Jid = normalized_chat_jid
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid chat JID '{}': {}", normalized_chat_jid, e))?;

        let is_group = chat.is_group();
        let normalized_participant = if is_group {
            if is_from_me {
                None
            } else {
                target_sender_jid.map(|s| {
                    if s.contains('@') {
                        s.to_string()
                    } else {
                        super::phone::phone_to_direct_jid(s)
                    }
                })
            }
        } else {
            None
        };

        let key = whatsapp_rust::prelude::wa::MessageKey {
            remote_jid: Some(chat.to_string()),
            from_me: Some(is_from_me),
            id: Some(target_message_id.to_string()),
            participant: normalized_participant,
        };

        self.client
            .send_reaction(&chat, key, emoji)
            .await
            .map_err(|e| anyhow::anyhow!("WhatsApp send reaction error: {}", e))?;

        Ok(())
    }

    async fn download_media(
        &self,
        params: &whatsapp_rust::download::DownloadParams,
    ) -> Result<Vec<u8>> {
        self.client
            .download(params)
            .await
            .context("WhatsApp media download error")
    }

    async fn request_media_reupload(
        &self,
        request: &WhatsAppMediaReuploadRequest,
    ) -> Result<WhatsAppMediaReuploadResult> {
        let chat_jid: whatsapp_rust::Jid = request.chat_jid.parse().map_err(|error| {
            anyhow::anyhow!(
                "Invalid media reupload chat JID '{}': {error}",
                request.chat_jid
            )
        })?;
        let participant_jid = request
            .participant_jid
            .as_deref()
            .map(str::parse::<whatsapp_rust::Jid>)
            .transpose()
            .map_err(|error| anyhow::anyhow!("Invalid media participant JID: {error}"))?;
        let upstream_request = whatsapp_rust::MediaReuploadRequest {
            msg_id: &request.provider_message_id,
            chat_jid: &chat_jid,
            media_key: &request.media_key,
            is_from_me: request.is_from_me,
            participant: participant_jid.as_ref(),
        };
        let result = self
            .client
            .media_reupload()
            .request(&upstream_request)
            .await
            .context("WhatsApp media reupload request failed")?;
        Ok(match result {
            whatsapp_rust::MediaRetryResult::Success { direct_path } => {
                WhatsAppMediaReuploadResult::Success { direct_path }
            }
            whatsapp_rust::MediaRetryResult::NotFound => WhatsAppMediaReuploadResult::NotFound,
            whatsapp_rust::MediaRetryResult::DecryptionError => {
                WhatsAppMediaReuploadResult::DecryptionError
            }
            whatsapp_rust::MediaRetryResult::GeneralError => {
                WhatsAppMediaReuploadResult::GeneralError
            }
        })
    }

    async fn get_group_info(&self, group_jid: &str) -> Result<whatsapp_rust::GroupMetadata> {
        let jid: whatsapp_rust::Jid = group_jid
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid group JID '{}': {}", group_jid, e))?;

        let outcome = self
            .client
            .execute(whatsapp_rust::wacore::iq::groups::GroupQueryIq::new(&jid))
            .await
            .map_err(|e| anyhow::anyhow!("WhatsApp get_group_info IQ error: {}", e))?;

        match outcome {
            whatsapp_rust::wacore::iq::groups::GroupInfoOutcome::Full(group) => {
                Ok(whatsapp_rust::GroupMetadata::from(*group))
            }
            whatsapp_rust::wacore::iq::groups::GroupInfoOutcome::NotModified => {
                Err(anyhow::anyhow!("Group query returned NotModified"))
            }
        }
    }

    async fn is_connected(&self) -> bool {
        self.client.is_connected()
    }

    async fn disconnect(&self) -> Result<()> {
        if let Some(handle) = self.bot_handle.lock().await.take() {
            handle.shutdown().await;
        }
        Ok(())
    }
}

#[cfg(test)]
pub use mock_client::*;

/// Test-only doubles. Compiled solely under `cfg(test)` so a production binary never
/// contains a mock WhatsApp client.
#[cfg(test)]
mod mock_client {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

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
        pub file_name: Option<String>,
        pub mime_type: Option<String>,
    }

    /// Recorded payload for sent reaction in `MockWhatsAppClient`.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct MockSentReaction {
        pub chat_jid: String,
        pub target_message_id: String,
        pub target_sender_jid: Option<String>,
        pub emoji: String,
        pub is_from_me: bool,
    }

    /// Deterministic mock implementation of `WhatsAppClient` for testing.
    #[derive(Clone)]
    pub struct MockWhatsAppClient {
        pub sent_messages: Arc<Mutex<Vec<MockSentTextMessage>>>,
        pub sent_media: Arc<Mutex<Vec<MockSentMediaMessage>>>,
        pub sent_reactions: Arc<Mutex<Vec<MockSentReaction>>>,
        pub next_send_result: Arc<Mutex<Option<Result<WhatsAppSendResponse>>>>,
        pub next_reaction_result: Arc<Mutex<Option<Result<()>>>>,
        pub download_results: Arc<Mutex<VecDeque<Result<Vec<u8>>>>>,
        pub media_reupload_requests: Arc<Mutex<Vec<WhatsAppMediaReuploadRequest>>>,
        pub next_media_reupload_result: Arc<Mutex<Option<Result<WhatsAppMediaReuploadResult>>>>,
        pub next_group_info_result: Arc<Mutex<Option<Result<whatsapp_rust::GroupMetadata>>>>,
        pub group_info_results:
            Arc<Mutex<std::collections::HashMap<String, Result<whatsapp_rust::GroupMetadata>>>>,
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
                download_results: Arc::new(Mutex::new(VecDeque::new())),
                media_reupload_requests: Arc::new(Mutex::new(Vec::new())),
                next_media_reupload_result: Arc::new(Mutex::new(None)),
                next_group_info_result: Arc::new(Mutex::new(None)),
                group_info_results: Arc::new(Mutex::new(std::collections::HashMap::new())),
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

        pub fn set_download_results(&self, results: Vec<Result<Vec<u8>>>) {
            *self.download_results.lock().unwrap() = results.into();
        }

        pub fn set_next_media_reupload_result(&self, result: Result<WhatsAppMediaReuploadResult>) {
            *self.next_media_reupload_result.lock().unwrap() = Some(result);
        }

        pub fn set_next_group_info_result(&self, res: Result<whatsapp_rust::GroupMetadata>) {
            *self.next_group_info_result.lock().unwrap() = Some(res);
        }

        pub fn set_group_info(
            &self,
            jid: impl Into<String>,
            res: Result<whatsapp_rust::GroupMetadata>,
        ) {
            self.group_info_results
                .lock()
                .unwrap()
                .insert(jid.into(), res);
        }

        pub fn get_media_reupload_requests(&self) -> Vec<WhatsAppMediaReuploadRequest> {
            self.media_reupload_requests.lock().unwrap().clone()
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
            self.download_results.lock().unwrap().clear();
            self.media_reupload_requests.lock().unwrap().clear();
            *self.next_send_result.lock().unwrap() = None;
            *self.next_reaction_result.lock().unwrap() = None;
            *self.next_media_reupload_result.lock().unwrap() = None;
            *self.next_group_info_result.lock().unwrap() = None;
            self.group_info_results.lock().unwrap().clear();
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
                    media_metadata: None,
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
            file_name: Option<&str>,
            mime_type: Option<&str>,
        ) -> Result<WhatsAppSendResponse> {
            self.sent_media.lock().unwrap().push(MockSentMediaMessage {
                recipient_jid: recipient_jid.to_string(),
                media_type: media_type.to_string(),
                media_data_or_url: media_data_or_url.to_string(),
                caption: caption.map(str::to_string),
                quote: quote.cloned(),
                file_name: file_name.map(str::to_string),
                mime_type: mime_type.map(str::to_string),
            });

            if let Some(res) = self.next_send_result.lock().unwrap().take() {
                res
            } else {
                Ok(WhatsAppSendResponse {
                    provider_message_id: format!("wa_media_{:016x}", rand::random::<u64>()),
                    timestamp: Some(chrono::Utc::now().to_rfc3339()),
                    media_metadata: Some(WhatsAppMediaUploadMetadata {
                        url: "https://mmg.whatsapp.net/mock/direct_path".to_string(),
                        direct_path: "/mock/direct_path".to_string(),
                        media_key: "bW9ja19tZWRpYV9rZXlfMzJieXRlc19mb3JfdGVzdA==".to_string(),
                        file_sha256: "bW9ja19zaGEyNTY=".to_string(),
                        file_enc_sha256: "bW9ja19lbmNfc2hhMjU2".to_string(),
                        file_length: 1024,
                    }),
                })
            }
        }

        async fn send_reaction(
            &self,
            chat_jid: &str,
            target_message_id: &str,
            target_sender_jid: Option<&str>,
            emoji: &str,
            is_from_me: bool,
        ) -> Result<()> {
            let normalized_chat_jid = if chat_jid.contains('@') {
                chat_jid.to_string()
            } else {
                super::super::phone::phone_to_direct_jid(chat_jid)
            };

            let chat: whatsapp_rust::Jid = normalized_chat_jid.parse().map_err(|e| {
                anyhow::anyhow!("Invalid chat JID '{}': {}", normalized_chat_jid, e)
            })?;

            let is_group = chat.is_group();
            let normalized_participant = if is_group {
                if is_from_me {
                    None
                } else {
                    target_sender_jid.map(|s| {
                        if s.contains('@') {
                            s.to_string()
                        } else {
                            super::super::phone::phone_to_direct_jid(s)
                        }
                    })
                }
            } else {
                None
            };

            if let Some(ref p) = normalized_participant {
                p.as_str()
                    .parse::<whatsapp_rust::Jid>()
                    .map_err(|e| anyhow::anyhow!("Invalid participant JID '{}': {}", p, e))?;
            }

            self.sent_reactions.lock().unwrap().push(MockSentReaction {
                chat_jid: chat.to_string(),
                target_message_id: target_message_id.to_string(),
                target_sender_jid: normalized_participant,
                emoji: emoji.to_string(),
                is_from_me,
            });

            if let Some(res) = self.next_reaction_result.lock().unwrap().take() {
                res
            } else {
                Ok(())
            }
        }

        async fn download_media(
            &self,
            _params: &whatsapp_rust::download::DownloadParams,
        ) -> Result<Vec<u8>> {
            if let Some(result) = self.download_results.lock().unwrap().pop_front() {
                return result;
            }
            // Deterministic dummy valid 1x1 PNG bytes for testing
            Ok(vec![
                0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
                0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
                0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78,
                0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
                0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
            ])
        }

        async fn request_media_reupload(
            &self,
            request: &WhatsAppMediaReuploadRequest,
        ) -> Result<WhatsAppMediaReuploadResult> {
            self.media_reupload_requests
                .lock()
                .unwrap()
                .push(request.clone());
            if let Some(result) = self.next_media_reupload_result.lock().unwrap().take() {
                result
            } else {
                Ok(WhatsAppMediaReuploadResult::Success {
                    direct_path: "/mock/refreshed-media.enc".to_string(),
                })
            }
        }

        async fn get_group_info(&self, group_jid: &str) -> Result<whatsapp_rust::GroupMetadata> {
            if let Some(res) = self.next_group_info_result.lock().unwrap().take() {
                return res;
            }
            if let Some(res) = self.group_info_results.lock().unwrap().get(group_jid) {
                return match res {
                    Ok(meta) => Ok(meta.clone()),
                    Err(e) => Err(anyhow::anyhow!("{}", e)),
                };
            }
            let jid: whatsapp_rust::Jid = group_jid
                .parse()
                .unwrap_or_else(|_| whatsapp_rust::Jid::group("120363000000000000"));
            Ok(whatsapp_rust::GroupMetadata {
                id: jid,
                subject: "Mock WhatsApp Group".to_string(),
                participants: vec![],
                ..Default::default()
            })
        }

        async fn is_connected(&self) -> bool {
            self.connected.load(Ordering::SeqCst)
        }

        async fn disconnect(&self) -> Result<()> {
            self.connected.store(false, Ordering::SeqCst);
            Ok(())
        }
    }
}
