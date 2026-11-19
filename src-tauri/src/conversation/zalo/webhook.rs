//! Batched webhook that reports new inbound Zalo messages.
//!
//! Configuration comes only from the environment:
//!
//! * `RUSTSALE_ZALO_WEBHOOK_URL`: target URL. Empty or missing disables the
//!   feature. The URL may carry a secret and is never logged.
//! * `RUSTSALE_ZALO_WEBHOOK_INCLUDE_GROUPS`: `1` also reports group messages.
//!   Anything else (the default) reports direct messages only.
//!
//! # Design
//!
//! The Zalo listener only calls [`ZaloWebhook::notify`], which does a
//! non-blocking `try_send` into a bounded channel and returns at once. A single
//! Tokio worker owns all batching state, so there are no locks held across
//! `.await`. Messages are grouped per conversation and sent as one POST when the
//! conversation has been quiet for [`WebhookTuning::quiet_period`] or when
//! [`WebhookTuning::max_wait`] has passed since the first message of the batch.
//!
//! Memory is bounded: a channel of [`CHANNEL_CAPACITY`] items, at most
//! [`MAX_PENDING_CONVERSATIONS`] open batches, at most [`MAX_MESSAGES_PER_BATCH`]
//! messages per batch and at most [`MAX_TEXT_CHARS`] characters per message.
//! Anything beyond those limits is dropped and logged at `warn` (without URL,
//! text, names or UIDs).
//!
//! Delivery is best effort: 10 s per attempt, 3 attempts, retrying only on
//! network errors, timeouts, HTTP 5xx and HTTP 429. A failed delivery is logged
//! and dropped; it can never block or break Zalo sync.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::Instant;

/// Environment variable holding the webhook URL.
pub const WEBHOOK_URL_ENV: &str = "RUSTSALE_ZALO_WEBHOOK_URL";
/// Environment variable that opts group messages in when set to `1`.
pub const WEBHOOK_INCLUDE_GROUPS_ENV: &str = "RUSTSALE_ZALO_WEBHOOK_INCLUDE_GROUPS";

/// Quiet time after the last message before a batch is sent.
const QUIET_PERIOD: Duration = Duration::from_secs(120);
/// Longest a batch may wait, measured from its first message.
const MAX_WAIT: Duration = Duration::from_secs(180);
/// Hard limit for one POST attempt (connect + response).
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(10);
/// Waits before attempt 2 and attempt 3.
const RETRY_DELAYS: [Duration; 2] = [Duration::from_secs(2), Duration::from_secs(8)];
/// Capacity of the listener-to-worker channel. Chosen above the open-batch
/// limit so a burst across many conversations is absorbed before shedding.
const CHANNEL_CAPACITY: usize = 1024;
/// Conversations that may hold an open batch at once.
const MAX_PENDING_CONVERSATIONS: usize = 1000;
/// Messages kept per batch.
const MAX_MESSAGES_PER_BATCH: usize = 50;
/// Characters kept per message; longer text is truncated.
const MAX_TEXT_CHARS: usize = 2000;
/// POSTs in flight at the same time; keeps a flush storm from opening
/// hundreds of connections.
const MAX_CONCURRENT_DELIVERIES: usize = 4;

/// Timing knobs. [`Default`] is the production setting; tests shrink it.
#[derive(Debug, Clone, Copy)]
pub struct WebhookTuning {
    /// Quiet time after the last message before a batch is sent.
    pub quiet_period: Duration,
    /// Longest wait since the first message of a batch.
    pub max_wait: Duration,
    /// Timeout of one POST attempt.
    pub attempt_timeout: Duration,
    /// Waits before attempt 2 and attempt 3 (jitter of up to 20 % is added).
    pub retry_delays: [Duration; 2],
}

impl Default for WebhookTuning {
    fn default() -> Self {
        Self {
            quiet_period: QUIET_PERIOD,
            max_wait: MAX_WAIT,
            attempt_timeout: ATTEMPT_TIMEOUT,
            retry_delays: RETRY_DELAYS,
        }
    }
}

/// One new inbound message handed over by the Zalo listener.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZaloWebhookMessage {
    pub conversation_id: i64,
    pub channel_account_id: i64,
    pub sender_uid: Option<String>,
    pub sender_name: Option<String>,
    /// Stored content type, for example `text`, `image`, `sticker`.
    pub content_type: String,
    /// Stored text (or caption); `None` when the message has none.
    pub text: Option<String>,
    /// RFC 3339 timestamp of the message.
    pub ts: String,
    pub is_group: bool,
}

/// JSON body of one POST. Field names are part of the receiver contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZaloWebhookPayload {
    pub conversation_id: i64,
    pub channel_account_id: i64,
    /// Sender of the last message of the batch.
    pub sender_uid: Option<String>,
    /// Sender name of the last message of the batch.
    pub sender_name: Option<String>,
    /// Message texts joined with `\n`; non-text messages appear as `[image]`.
    pub text: String,
    /// Timestamp (RFC 3339) of the last message of the batch.
    pub ts: String,
    pub is_group: bool,
    pub message_count: usize,
    /// Always `false`; reserved so receivers can tell real sends from probes.
    pub test: bool,
}

/// Sender of webhook notifications. Cheap to clone; clones share one worker.
#[derive(Clone)]
pub struct ZaloWebhook {
    inner: Option<Arc<Inner>>,
}

struct Inner {
    url: String,
    include_groups: bool,
    tuning: WebhookTuning,
    /// Sender of the running worker; recreated if the worker ever stops.
    worker: Mutex<Option<mpsc::Sender<ZaloWebhookMessage>>>,
}

impl std::fmt::Debug for ZaloWebhook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the URL.
        f.debug_struct("ZaloWebhook")
            .field("enabled", &self.inner.is_some())
            .finish()
    }
}

impl ZaloWebhook {
    /// A webhook that never sends anything.
    pub fn disabled() -> Self {
        Self { inner: None }
    }

    /// Reads the two environment variables; disabled when the URL is unset or blank.
    pub fn from_env() -> Self {
        let include_groups = std::env::var(WEBHOOK_INCLUDE_GROUPS_ENV)
            .map(|value| value.trim() == "1")
            .unwrap_or(false);
        Self::new(
            std::env::var(WEBHOOK_URL_ENV).ok().as_deref(),
            include_groups,
            WebhookTuning::default(),
        )
    }

    /// Builds a webhook for `url`; `None`, a blank value or a non-http(s)
    /// scheme disables it.
    pub fn new(url: Option<&str>, include_groups: bool, tuning: WebhookTuning) -> Self {
        let Some(url) = url.map(str::trim).filter(|value| !value.is_empty()) else {
            return Self::disabled();
        };
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            tracing::warn!("zalo.webhook.config.invalid_scheme: webhook disabled");
            return Self::disabled();
        }
        Self {
            inner: Some(Arc::new(Inner {
                url: url.to_string(),
                include_groups,
                tuning,
                worker: Mutex::new(None),
            })),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    /// Queues `message` for batching. Never blocks and never fails the caller.
    ///
    /// Group messages are ignored unless groups were enabled. Does nothing
    /// when the webhook is disabled or when no Tokio runtime is available to
    /// host the worker. When the queue is full the message is dropped and a
    /// `warn` is logged.
    pub fn notify(&self, message: ZaloWebhookMessage) {
        let Some(inner) = &self.inner else {
            return;
        };
        if message.is_group && !inner.include_groups {
            return;
        }
        let Some(sender) = inner.sender() else {
            tracing::debug!("zalo.webhook.skipped: no tokio runtime");
            return;
        };
        let conversation_id = message.conversation_id;
        match sender.try_send(message) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!(conversation_id, "zalo.webhook.dropped: queue full");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                inner.clear_worker();
                tracing::warn!(conversation_id, "zalo.webhook.dropped: worker stopped");
            }
        }
    }
}

impl Inner {
    /// Returns the worker's sender, starting the worker on first use.
    fn sender(self: &Arc<Self>) -> Option<mpsc::Sender<ZaloWebhookMessage>> {
        let mut guard = match self.worker.lock() {
            Ok(guard) => guard,
            // The slot only holds a Sender, so a poisoned lock is still valid.
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(sender) = guard.as_ref() {
            if !sender.is_closed() {
                return Some(sender.clone());
            }
        }
        let handle = tokio::runtime::Handle::try_current().ok()?;
        let client = match reqwest::Client::builder()
            .timeout(self.tuning.attempt_timeout)
            .build()
        {
            Ok(client) => client,
            Err(error) => {
                tracing::warn!(error = %error.without_url(), "zalo.webhook.client_failed");
                return None;
            }
        };
        let (sender, receiver) = mpsc::channel(CHANNEL_CAPACITY);
        handle.spawn(run_worker(
            receiver,
            Delivery {
                url: self.url.clone(),
                client,
                tuning: self.tuning,
            },
        ));
        *guard = Some(sender.clone());
        Some(sender)
    }

    fn clear_worker(&self) {
        let mut guard = match self.worker.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *guard = None;
    }
}

/// Messages of one conversation waiting to be sent.
struct Batch {
    first_at: Instant,
    last_at: Instant,
    messages: Vec<ZaloWebhookMessage>,
    dropped: usize,
}

impl Batch {
    fn due_at(&self, tuning: &WebhookTuning) -> Instant {
        (self.last_at + tuning.quiet_period).min(self.first_at + tuning.max_wait)
    }
}

#[derive(Clone)]
struct Delivery {
    url: String,
    client: reqwest::Client,
    tuning: WebhookTuning,
}

/// The single batching worker. Ends when every sender is gone.
async fn run_worker(mut receiver: mpsc::Receiver<ZaloWebhookMessage>, delivery: Delivery) {
    let permits = Arc::new(Semaphore::new(MAX_CONCURRENT_DELIVERIES));
    let mut batches: HashMap<i64, Batch> = HashMap::new();
    loop {
        let next_due = batches
            .values()
            .map(|batch| batch.due_at(&delivery.tuning))
            .min();
        let incoming = match next_due {
            Some(deadline) => {
                tokio::select! {
                    message = receiver.recv() => Some(message),
                    () = tokio::time::sleep_until(deadline) => None,
                }
            }
            None => Some(receiver.recv().await),
        };
        match incoming {
            Some(Some(message)) => add_to_batches(&mut batches, message),
            Some(None) => {
                // Channel closed: flush what is left, then stop.
                let remaining: Vec<(i64, Batch)> = batches.drain().collect();
                for (conversation_id, batch) in remaining {
                    spawn_delivery(&delivery, &permits, conversation_id, batch);
                }
                return;
            }
            None => {
                let now = Instant::now();
                let due: Vec<i64> = batches
                    .iter()
                    .filter(|(_, batch)| batch.due_at(&delivery.tuning) <= now)
                    .map(|(conversation_id, _)| *conversation_id)
                    .collect();
                for conversation_id in due {
                    if let Some(batch) = batches.remove(&conversation_id) {
                        spawn_delivery(&delivery, &permits, conversation_id, batch);
                    }
                }
            }
        }
    }
}

fn add_to_batches(batches: &mut HashMap<i64, Batch>, message: ZaloWebhookMessage) {
    let now = Instant::now();
    let conversation_id = message.conversation_id;
    if !batches.contains_key(&conversation_id) && batches.len() >= MAX_PENDING_CONVERSATIONS {
        tracing::warn!(
            conversation_id,
            "zalo.webhook.dropped: too many pending conversations"
        );
        return;
    }
    let batch = batches.entry(conversation_id).or_insert_with(|| Batch {
        first_at: now,
        last_at: now,
        messages: Vec::new(),
        dropped: 0,
    });
    if batch.messages.len() >= MAX_MESSAGES_PER_BATCH {
        batch.dropped += 1;
        // Warn once per batch, on the first overflow.
        if batch.dropped == 1 {
            tracing::warn!(
                conversation_id,
                limit = MAX_MESSAGES_PER_BATCH,
                "zalo.webhook.dropped: batch full"
            );
        }
        return;
    }
    batch.last_at = now;
    batch.messages.push(message);
}

fn spawn_delivery(
    delivery: &Delivery,
    permits: &Arc<Semaphore>,
    conversation_id: i64,
    batch: Batch,
) {
    let Some(payload) = build_payload(conversation_id, batch.messages) else {
        return;
    };
    let delivery = delivery.clone();
    let permits = Arc::clone(permits);
    tokio::spawn(async move {
        let Ok(_permit) = permits.acquire_owned().await else {
            return;
        };
        deliver(&delivery, &payload).await;
    });
}

/// Joins a batch into one payload; `None` for an empty batch.
pub(crate) fn build_payload(
    conversation_id: i64,
    messages: Vec<ZaloWebhookMessage>,
) -> Option<ZaloWebhookPayload> {
    let message_count = messages.len();
    let last = messages.last()?;
    let channel_account_id = last.channel_account_id;
    let sender_uid = last.sender_uid.clone();
    let sender_name = last.sender_name.clone();
    let ts = last.ts.clone();
    let is_group = last.is_group;
    let text = messages
        .iter()
        .map(message_line)
        .collect::<Vec<_>>()
        .join("\n");
    Some(ZaloWebhookPayload {
        conversation_id,
        channel_account_id,
        sender_uid,
        sender_name,
        text,
        ts,
        is_group,
        message_count,
        test: false,
    })
}

/// Text of one message: its text, or `[type]` (plus caption) for non-text.
fn message_line(message: &ZaloWebhookMessage) -> String {
    let text = message
        .text
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(MAX_TEXT_CHARS).collect::<String>());
    match (message.content_type.as_str(), text) {
        ("text", Some(text)) => text,
        (kind, Some(text)) => format!("[{kind}] {text}"),
        (kind, None) => format!("[{kind}]"),
    }
}

/// Outcome of one POST attempt.
enum Attempt {
    Delivered(u16),
    /// Worth another try: network error, timeout, 5xx or 429.
    Retryable(Option<u16>),
    /// Not worth another try: any other HTTP status.
    Rejected(u16),
}

async fn attempt_once(delivery: &Delivery, payload: &ZaloWebhookPayload) -> Attempt {
    match delivery
        .client
        .post(&delivery.url)
        .json(payload)
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status();
            if status.is_success() {
                Attempt::Delivered(status.as_u16())
            } else if status.is_server_error() || status.as_u16() == 429 {
                Attempt::Retryable(Some(status.as_u16()))
            } else {
                Attempt::Rejected(status.as_u16())
            }
        }
        // Transport-level failure (connect, timeout, reset, ...).
        Err(_) => Attempt::Retryable(None),
    }
}

/// Delay before the next attempt: the base delay plus up to 20 % jitter.
fn jittered(base: Duration) -> Duration {
    let jitter_ms = u64::try_from(base.as_millis() / 5).unwrap_or(0);
    let extra = if jitter_ms == 0 {
        0
    } else {
        rand::random::<u64>() % (jitter_ms + 1)
    };
    base + Duration::from_millis(extra)
}

async fn deliver(delivery: &Delivery, payload: &ZaloWebhookPayload) {
    let conversation_id = payload.conversation_id;
    let message_count = payload.message_count;
    let mut attempts: usize = 0;
    let failed_status = loop {
        attempts += 1;
        match attempt_once(delivery, payload).await {
            Attempt::Delivered(status) => {
                tracing::info!(
                    conversation_id,
                    message_count,
                    attempts,
                    status,
                    "zalo.webhook.delivered"
                );
                return;
            }
            Attempt::Rejected(status) => {
                tracing::warn!(
                    conversation_id,
                    message_count,
                    attempts,
                    status,
                    "zalo.webhook.failed"
                );
                return;
            }
            Attempt::Retryable(status) => {
                let Some(delay) = delivery.tuning.retry_delays.get(attempts - 1) else {
                    // 0 means no HTTP response (network error or timeout).
                    break status.unwrap_or(0);
                };
                tokio::time::sleep(jittered(*delay)).await;
            }
        }
    };
    tracing::warn!(
        conversation_id,
        message_count,
        attempts,
        status = failed_status,
        "zalo.webhook.failed"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(kind: &str, text: Option<&str>) -> ZaloWebhookMessage {
        ZaloWebhookMessage {
            conversation_id: 7,
            channel_account_id: 1,
            sender_uid: Some("u1".into()),
            sender_name: Some("A".into()),
            content_type: kind.into(),
            text: text.map(str::to_string),
            ts: "2026-11-18T10:00:00+00:00".into(),
            is_group: false,
        }
    }

    #[test]
    fn non_text_messages_become_placeholders() {
        assert_eq!(message_line(&message("image", None)), "[image]");
        assert_eq!(message_line(&message("sticker", None)), "[sticker]");
        assert_eq!(message_line(&message("image", Some("ảnh"))), "[image] ảnh");
        assert_eq!(message_line(&message("text", Some(" hi "))), "hi");
    }

    #[test]
    fn long_text_is_truncated() {
        let long = "a".repeat(MAX_TEXT_CHARS + 50);
        let line = message_line(&message("text", Some(&long)));
        assert_eq!(line.chars().count(), MAX_TEXT_CHARS);
    }

    #[test]
    fn payload_joins_text_and_uses_last_message_metadata() {
        let mut last = message("text", Some("hai"));
        last.ts = "2026-11-18T10:05:00+00:00".into();
        let payload =
            build_payload(7, vec![message("text", Some("một")), last]).expect("non-empty batch");
        assert_eq!(payload.text, "một\nhai");
        assert_eq!(payload.message_count, 2);
        assert_eq!(payload.ts, "2026-11-18T10:05:00+00:00");
        assert!(!payload.test);
        assert!(build_payload(7, Vec::new()).is_none());
    }

    #[test]
    fn debug_output_never_contains_the_url() {
        let webhook = ZaloWebhook::new(
            Some("http://127.0.0.1:1/secret-token-123"),
            false,
            WebhookTuning::default(),
        );
        assert!(!format!("{webhook:?}").contains("secret-token-123"));
    }

    #[test]
    fn jitter_stays_within_twenty_percent() {
        for _ in 0..50 {
            let delay = jittered(Duration::from_secs(2));
            assert!(delay >= Duration::from_secs(2));
            assert!(delay <= Duration::from_millis(2400));
        }
    }
}
