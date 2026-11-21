//! Batched webhook that reports new inbound Zalo messages.
//!
//! Configuration comes only from the environment:
//!
//! * `RUSTSALE_ZALO_WEBHOOK_URL`: target URL. Empty or missing disables the
//!   feature. The URL may carry a secret and is never logged.
//! * `RUSTSALE_ZALO_WEBHOOK_INCLUDE_GROUPS`: `1` also reports group messages.
//!   Anything else (the default) reports direct messages only.
//! * `RUSTSALE_ZALO_WEBHOOK_ALLOWLIST_FILE`: path of the customer allowlist
//!   (default: `zalo-webhook-allowlist.csv` next to the database, normally
//!   `~/.local/share/com.rustsale.crm/`).
//!
//! # Customer allowlist file
//!
//! One customer per line: `PROJECT_CODE,senderUid,conversationId`. Blank lines
//! and lines starting with `#` are ignored; senderUid or conversationId may be
//! empty (not both). A message is reported only if its sender UID or its
//! conversation id matches some line, and the payload then carries that line's
//! `projectCode`. The file is re-read when its mtime or size changes (checked
//! at most every 30 s), so adding a customer needs no restart. It fails
//! closed: a missing, unreadable or rule-less file means nothing is sent. The
//! filter lives only here; messages are always stored in the database. See
//! `docs/zalo-webhook-allowlist.example.csv`.
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
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use serde::Serialize;
use tokio::sync::{mpsc, Semaphore};
use tokio::time::Instant;

/// Environment variable holding the webhook URL.
pub const WEBHOOK_URL_ENV: &str = "RUSTSALE_ZALO_WEBHOOK_URL";
/// Environment variable that opts group messages in when set to `1`.
pub const WEBHOOK_INCLUDE_GROUPS_ENV: &str = "RUSTSALE_ZALO_WEBHOOK_INCLUDE_GROUPS";
/// Environment variable holding the allowlist file path.
pub const WEBHOOK_ALLOWLIST_FILE_ENV: &str = "RUSTSALE_ZALO_WEBHOOK_ALLOWLIST_FILE";
/// File name of the allowlist inside the app data directory.
pub const WEBHOOK_ALLOWLIST_FILE_NAME: &str = "zalo-webhook-allowlist.csv";
/// Longest accepted sender UID; anything longer is treated as invalid.
const MAX_ALLOWED_UID_CHARS: usize = 64;
/// Longest accepted project code.
const MAX_PROJECT_CODE_CHARS: usize = 32;
/// Allowlist files larger than this are refused (fail closed).
const MAX_ALLOWLIST_BYTES: u64 = 256 * 1024;
/// Rules kept from one allowlist file.
const MAX_ALLOWLIST_RULES: usize = 2000;
/// Invalid line numbers named in one warning.
const MAX_REPORTED_BAD_LINES: usize = 20;
/// Minimum time between two `stat` calls on the allowlist file.
const ALLOWLIST_RECHECK: Duration = Duration::from_secs(30);

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
    /// Stored attachments of this message (ids only; never provider URLs).
    pub attachments: Vec<WebhookAttachment>,
    /// Project code of the allowlist line that matched; set by
    /// [`ZaloWebhook::notify`], so listeners leave it `None`.
    pub project_code: Option<String>,
}

/// Reference to a stored attachment, readable through the local API `read_attachment`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookAttachment {
    pub message_id: i64,
    pub attachment_id: i64,
    pub kind: String,
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
    /// Attachments of every message in the batch, in message order.
    pub attachments: Vec<WebhookAttachment>,
    /// Always `false`; reserved so receivers can tell real sends from probes.
    pub test: bool,
    /// Project code of the matching allowlist line; omitted when no allowlist
    /// is in use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_code: Option<String>,
}

/// One allowlist line: a customer, identified by sender UID and/or conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AllowRule {
    project_code: String,
    sender_uid: Option<String>,
    conversation_id: Option<i64>,
}

impl AllowRule {
    fn matches(&self, message: &ZaloWebhookMessage) -> bool {
        self.conversation_id == Some(message.conversation_id)
            || match (&self.sender_uid, message.sender_uid.as_deref()) {
                (Some(rule_uid), Some(uid)) => rule_uid == uid.trim(),
                _ => false,
            }
    }
}

/// Parses the allowlist text. Returns the valid rules and the 1-based numbers
/// of the lines that were skipped as malformed.
fn parse_allowlist(text: &str) -> (Vec<AllowRule>, Vec<usize>) {
    let mut rules = Vec::new();
    let mut bad_lines = Vec::new();
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match parse_rule(line) {
            Some(rule) if rules.len() < MAX_ALLOWLIST_RULES => rules.push(rule),
            Some(_) | None => bad_lines.push(index + 1),
        }
    }
    (rules, bad_lines)
}

fn parse_rule(line: &str) -> Option<AllowRule> {
    let mut columns = line.split(',').map(str::trim);
    let (code, uid, conversation) = (columns.next()?, columns.next()?, columns.next()?);
    if columns.next().is_some() {
        return None;
    }
    let project_code_ok = !code.is_empty()
        && code.chars().count() <= MAX_PROJECT_CODE_CHARS
        && code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if !project_code_ok {
        return None;
    }
    let sender_uid = if uid.is_empty() {
        None
    } else if uid.chars().count() <= MAX_ALLOWED_UID_CHARS
        && uid.chars().all(|c| c.is_ascii_graphic())
    {
        Some(uid.to_string())
    } else {
        return None;
    };
    let conversation_id = if conversation.is_empty() {
        None
    } else {
        Some(conversation.parse::<i64>().ok()?)
    };
    if sender_uid.is_none() && conversation_id.is_none() {
        return None;
    }
    Some(AllowRule {
        project_code: code.to_string(),
        sender_uid,
        conversation_id,
    })
}

/// Which messages may be reported.
#[derive(Debug)]
pub enum WebhookAllowlist {
    /// No filtering (tests and explicit wiring only).
    Open,
    /// Customer file; fails closed.
    File(FileAllowlist),
}

impl WebhookAllowlist {
    /// Allowlist backed by the file at `path`, re-checked at most every `recheck`.
    pub fn file(path: PathBuf, recheck: Duration) -> Self {
        Self::File(FileAllowlist {
            path,
            recheck,
            state: Mutex::new(FileState::default()),
        })
    }

    /// Project code of the matching line, or `None` when the message must not
    /// be reported. [`WebhookAllowlist::Open`] reports everything without a code.
    fn decide(&self, message: &ZaloWebhookMessage) -> Option<Option<String>> {
        match self {
            Self::Open => Some(None),
            Self::File(file) => file.lookup(message).map(Some),
        }
    }
}

/// Allowlist file plus its cached rules.
#[derive(Debug)]
pub struct FileAllowlist {
    path: PathBuf,
    recheck: Duration,
    state: Mutex<FileState>,
}

#[derive(Debug, Default)]
struct FileState {
    rules: Vec<AllowRule>,
    /// `(mtime, size)` of the file the rules were read from.
    stamp: Option<(Option<SystemTime>, u64)>,
    checked_at: Option<std::time::Instant>,
    /// Whether the last load attempt failed; used to warn once per failure.
    failed: bool,
}

impl FileAllowlist {
    fn lookup(&self, message: &ZaloWebhookMessage) -> Option<String> {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            // Plain data; a poisoned lock is still consistent enough to use.
            Err(poisoned) => poisoned.into_inner(),
        };
        self.refresh(&mut state);
        state
            .rules
            .iter()
            .find(|rule| rule.matches(message))
            .map(|rule| rule.project_code.clone())
    }

    /// Re-reads the file when it changed. Blocking, but it is one `stat` per
    /// `recheck` and a read of at most [`MAX_ALLOWLIST_BYTES`] on change.
    fn refresh(&self, state: &mut FileState) {
        if state
            .checked_at
            .is_some_and(|checked| checked.elapsed() < self.recheck)
        {
            return;
        }
        state.checked_at = Some(std::time::Instant::now());
        let metadata = match std::fs::metadata(&self.path) {
            Ok(metadata) if metadata.is_file() && metadata.len() <= MAX_ALLOWLIST_BYTES => metadata,
            Ok(_) => return Self::fail(state, "not a regular file or too large"),
            Err(error) => return Self::fail(state, &format!("stat: {:?}", error.kind())),
        };
        let stamp = (metadata.modified().ok(), metadata.len());
        if state.stamp == Some(stamp) {
            return;
        }
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) => return Self::fail(state, &format!("read: {:?}", error.kind())),
        };
        let (rules, bad_lines) = parse_allowlist(&text);
        if !bad_lines.is_empty() {
            let shown: Vec<usize> = bad_lines
                .iter()
                .copied()
                .take(MAX_REPORTED_BAD_LINES)
                .collect();
            tracing::warn!(
                invalid_lines = ?shown,
                invalid_total = bad_lines.len(),
                "zalo.webhook.allowlist.invalid_lines_skipped"
            );
        }
        if rules.is_empty() {
            tracing::warn!("zalo.webhook.allowlist.no_valid_rules: nothing will be sent");
        } else {
            tracing::info!(rules = rules.len(), "zalo.webhook.allowlist.loaded");
        }
        state.rules = rules;
        state.stamp = Some(stamp);
        state.failed = false;
    }

    /// Fail closed: forget the rules and warn once until the file loads again.
    fn fail(state: &mut FileState, reason: &str) {
        state.rules.clear();
        state.stamp = None;
        if !state.failed {
            tracing::warn!(
                reason,
                "zalo.webhook.allowlist.unavailable: nothing will be sent"
            );
        }
        state.failed = true;
    }
}

/// Sender of webhook notifications. Cheap to clone; clones share one worker.
#[derive(Clone)]
pub struct ZaloWebhook {
    inner: Option<Arc<Inner>>,
}

struct Inner {
    url: String,
    include_groups: bool,
    allowlist: WebhookAllowlist,
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

/// Default allowlist location: next to the database. An empty path (no data
/// directory and no `HOME`) can never be read, so the filter fails closed.
fn default_allowlist_path(data_dir: Option<&Path>) -> PathBuf {
    if let Some(dir) = data_dir.filter(|dir| !dir.as_os_str().is_empty()) {
        return dir.join(WEBHOOK_ALLOWLIST_FILE_NAME);
    }
    std::env::var_os("HOME")
        .map(|home| {
            PathBuf::from(home)
                .join(".local/share/com.rustsale.crm")
                .join(WEBHOOK_ALLOWLIST_FILE_NAME)
        })
        .unwrap_or_default()
}

impl ZaloWebhook {
    /// A webhook that never sends anything.
    pub fn disabled() -> Self {
        Self { inner: None }
    }

    /// Reads the environment variables; disabled when the URL is unset or blank.
    ///
    /// The allowlist file is `RUSTSALE_ZALO_WEBHOOK_ALLOWLIST_FILE`, else
    /// `zalo-webhook-allowlist.csv` inside `data_dir` (the app data directory,
    /// where the database lives), else inside the default Linux data directory.
    pub fn from_env(data_dir: Option<&Path>) -> Self {
        let include_groups = std::env::var(WEBHOOK_INCLUDE_GROUPS_ENV)
            .map(|value| value.trim() == "1")
            .unwrap_or(false);
        let explicit = std::env::var(WEBHOOK_ALLOWLIST_FILE_ENV)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let path = explicit.unwrap_or_else(|| default_allowlist_path(data_dir));
        Self::new_with_allowlist(
            std::env::var(WEBHOOK_URL_ENV).ok().as_deref(),
            include_groups,
            WebhookAllowlist::file(path, ALLOWLIST_RECHECK),
            WebhookTuning::default(),
        )
    }

    /// Builds a webhook for `url`; `None`, a blank value or a non-http(s)
    /// scheme disables it.
    pub fn new(url: Option<&str>, include_groups: bool, tuning: WebhookTuning) -> Self {
        Self::new_with_allowlist(url, include_groups, WebhookAllowlist::Open, tuning)
    }

    /// Like [`ZaloWebhook::new`], with an allowlist applied after the group rule.
    /// [`ZaloWebhook::new`] itself reports everything.
    pub fn new_with_allowlist(
        url: Option<&str>,
        include_groups: bool,
        allowlist: WebhookAllowlist,
        tuning: WebhookTuning,
    ) -> Self {
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
                allowlist,
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
    /// Group messages are ignored unless groups were enabled, and when an
    /// allowlist is configured only listed senders/conversations pass. Does nothing
    /// when the webhook is disabled or when no Tokio runtime is available to
    /// host the worker. When the queue is full the message is dropped and a
    /// `warn` is logged.
    pub fn notify(&self, mut message: ZaloWebhookMessage) {
        let Some(inner) = &self.inner else {
            return;
        };
        if message.is_group && !inner.include_groups {
            return;
        }
        // Filter on the webhook only: the message is already stored by now.
        let Some(project_code) = inner.allowlist.decide(&message) else {
            tracing::debug!(
                conversation_id = message.conversation_id,
                "zalo.webhook.skipped: not on allowlist"
            );
            return;
        };
        message.project_code = project_code;
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
    let project_code = last.project_code.clone();
    let attachments = messages
        .iter()
        .flat_map(|message| message.attachments.iter().cloned())
        .collect();
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
        attachments,
        test: false,
        project_code,
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
        // Unsupported messages already carry their own text or bracketed label.
        ("unsupported", Some(text)) => text,
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
            attachments: Vec::new(),
            project_code: None,
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
    fn payload_lists_attachment_ids_of_every_message_without_urls() {
        let mut first = message("image", Some("cont này"));
        first.attachments = vec![WebhookAttachment {
            message_id: 11,
            attachment_id: 21,
            kind: "image".into(),
        }];
        let mut second = message("image", None);
        second.attachments = vec![
            WebhookAttachment {
                message_id: 12,
                attachment_id: 22,
                kind: "image".into(),
            },
            WebhookAttachment {
                message_id: 12,
                attachment_id: 23,
                kind: "file".into(),
            },
        ];
        let payload = build_payload(7, vec![first, second]).expect("non-empty batch");
        let value = serde_json::to_value(&payload).expect("serializes");
        assert_eq!(
            value["attachments"],
            serde_json::json!([
                {"messageId": 11, "attachmentId": 21, "kind": "image"},
                {"messageId": 12, "attachmentId": 22, "kind": "image"},
                {"messageId": 12, "attachmentId": 23, "kind": "file"},
            ])
        );
        assert!(!value.to_string().contains("http"));
    }

    #[test]
    fn payload_without_attachments_has_an_empty_list() {
        let payload = build_payload(7, vec![message("text", Some("hi"))]).expect("non-empty batch");
        let value = serde_json::to_value(&payload).expect("serializes");
        assert_eq!(value["attachments"], serde_json::json!([]));
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

    fn allow_message(conversation_id: i64, uid: Option<&str>) -> ZaloWebhookMessage {
        let mut value = message("text", Some("hi"));
        value.conversation_id = conversation_id;
        value.sender_uid = uid.map(str::to_string);
        value
    }

    fn rules_of(text: &str) -> Vec<AllowRule> {
        parse_allowlist(text).0
    }

    fn matching_code(rules: &[AllowRule], message: &ZaloWebhookMessage) -> Option<String> {
        rules
            .iter()
            .find(|rule| rule.matches(message))
            .map(|rule| rule.project_code.clone())
    }

    #[test]
    fn parses_comments_blanks_and_the_example_line() {
        let (rules, bad) = parse_allowlist(
            "\u{feff}# mã dự án, uid, conversation\n\nVTQA,7359857892895854028,2250596\r\n  # note\n",
        );
        assert!(bad.is_empty());
        assert_eq!(
            rules,
            vec![AllowRule {
                project_code: "VTQA".into(),
                sender_uid: Some("7359857892895854028".into()),
                conversation_id: Some(2_250_596),
            }]
        );
    }

    #[test]
    fn one_of_uid_or_conversation_may_be_empty_but_not_both() {
        let (rules, bad) = parse_allowlist("A,111,\nB,,22\nC,,\nD, ,  \n");
        assert_eq!(rules.len(), 2);
        assert_eq!(bad, vec![3, 4]);
    }

    #[test]
    fn malformed_lines_are_skipped_with_their_numbers() {
        let text = "VTQA,1,2\nonly-two,3\nA,1,2,4\nbad code!,1,2\nX,not number,abc\nY,bad uid,5\n,1,2\nOK,9,\n";
        let (rules, bad) = parse_allowlist(text);
        assert_eq!(rules.len(), 2);
        assert_eq!(bad, vec![2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn overlong_uid_and_code_are_rejected() {
        let long_uid = "1".repeat(MAX_ALLOWED_UID_CHARS + 1);
        let long_code = "A".repeat(MAX_PROJECT_CODE_CHARS + 1);
        let (rules, bad) = parse_allowlist(&format!("A,{long_uid},\n{long_code},1,\n"));
        assert!(rules.is_empty());
        assert_eq!(bad, vec![1, 2]);
    }

    #[test]
    fn rule_count_is_capped() {
        let text: String = (0..MAX_ALLOWLIST_RULES + 5)
            .map(|index| format!("P,{index},\n"))
            .collect();
        let (rules, bad) = parse_allowlist(&text);
        assert_eq!(rules.len(), MAX_ALLOWLIST_RULES);
        assert_eq!(bad.len(), 5);
    }

    #[test]
    fn matching_is_uid_or_conversation_and_first_line_wins() {
        let rules = rules_of("VTQA,111,5\nOTHER,111,\nTHIRD,,7\n");
        assert_eq!(
            matching_code(&rules, &allow_message(99, Some("111"))).as_deref(),
            Some("VTQA")
        );
        assert_eq!(
            matching_code(&rules, &allow_message(5, Some("zzz"))).as_deref(),
            Some("VTQA")
        );
        assert_eq!(
            matching_code(&rules, &allow_message(5, None)).as_deref(),
            Some("VTQA")
        );
        assert_eq!(
            matching_code(&rules, &allow_message(7, None)).as_deref(),
            Some("THIRD")
        );
        assert_eq!(
            matching_code(&rules, &allow_message(101_629, Some("other"))),
            None
        );
        assert_eq!(matching_code(&rules, &allow_message(101_629, None)), None);
    }

    #[test]
    fn empty_uid_column_never_matches_a_message_without_uid() {
        let rules = rules_of("A,,5\n");
        assert_eq!(matching_code(&rules, &allow_message(6, None)), None);
        assert_eq!(matching_code(&rules, &allow_message(6, Some(""))), None);
    }

    #[test]
    fn file_allowlist_fails_closed_and_reloads_on_change() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("allow.csv");
        let allowlist = WebhookAllowlist::file(path.clone(), Duration::ZERO);
        let probe = allow_message(1, Some("111"));

        // Missing file: nothing passes.
        assert_eq!(allowlist.decide(&probe), None);
        // Only junk: still nothing.
        std::fs::write(&path, "garbage\n# c\n").expect("write");
        assert_eq!(allowlist.decide(&probe), None);
        // A valid line appears: reloaded without recreating the allowlist.
        std::fs::write(&path, "VTQA,111,\n").expect("write");
        assert_eq!(allowlist.decide(&probe), Some(Some("VTQA".to_string())));
        // Another customer is added.
        std::fs::write(&path, "VTQA,111,\nACME,,42\n").expect("write");
        assert_eq!(
            allowlist.decide(&allow_message(42, None)),
            Some(Some("ACME".to_string()))
        );
        // The file disappears again: fail closed.
        std::fs::remove_file(&path).expect("remove");
        assert_eq!(allowlist.decide(&probe), None);
    }

    #[test]
    fn file_is_not_re_read_inside_the_recheck_window() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("allow.csv");
        std::fs::write(&path, "VTQA,111,\n").expect("write");
        let allowlist = WebhookAllowlist::file(path.clone(), Duration::from_secs(3600));
        let probe = allow_message(1, Some("111"));
        assert!(allowlist.decide(&probe).is_some());
        std::fs::remove_file(&path).expect("remove");
        // Still served from the cache until the window passes.
        assert!(allowlist.decide(&probe).is_some());
    }

    #[test]
    fn oversized_file_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("allow.csv");
        let filler = "#".repeat(usize::try_from(MAX_ALLOWLIST_BYTES).unwrap_or(0) + 1);
        std::fs::write(&path, format!("{filler}\nVTQA,111,\n")).expect("write");
        let allowlist = WebhookAllowlist::file(path, Duration::ZERO);
        assert_eq!(allowlist.decide(&allow_message(1, Some("111"))), None);
    }

    #[test]
    fn default_path_is_next_to_the_database() {
        let path = default_allowlist_path(Some(Path::new("/data/app")));
        assert_eq!(path, Path::new("/data/app/zalo-webhook-allowlist.csv"));
    }

    #[test]
    fn open_allowlist_reports_everything_without_a_code() {
        assert_eq!(
            WebhookAllowlist::Open.decide(&allow_message(1, None)),
            Some(None)
        );
    }

    #[test]
    fn payload_carries_project_code_only_when_set() {
        let mut matched = message("text", Some("hi"));
        matched.project_code = Some("VTQA".into());
        let with = build_payload(7, vec![matched]).expect("non-empty");
        assert_eq!(
            serde_json::to_value(&with).expect("json")["projectCode"],
            "VTQA"
        );
        let without = build_payload(7, vec![message("text", Some("hi"))]).expect("non-empty");
        assert!(serde_json::to_value(&without)
            .expect("json")
            .get("projectCode")
            .is_none());
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
