use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Configuration parameters for anti-ban rate limiting and burst management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrottlingConfig {
    pub min_delay_secs: u64,
    pub max_delay_secs: u64,
    pub burst_limit: u32,
    pub cooldown_secs: u64,
    pub daily_quota: u32,
}

impl Default for ThrottlingConfig {
    fn default() -> Self {
        Self {
            min_delay_secs: 3,
            max_delay_secs: 12,
            burst_limit: 15,
            cooldown_secs: 60,
            daily_quota: 250,
        }
    }
}

/// Dynamic runtime usage metrics per account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountStats {
    pub account_key: String,
    pub sent_today: u32,
    pub last_sent_epoch_secs: u64,
    pub current_burst_count: u32,
    pub last_burst_epoch_secs: u64,
    pub day_tag: String,
}

impl AccountStats {
    pub fn new(account_key: &str) -> Self {
        Self {
            account_key: account_key.to_string(),
            sent_today: 0,
            last_sent_epoch_secs: 0,
            current_burst_count: 0,
            last_burst_epoch_secs: 0,
            day_tag: Self::current_day_tag(),
        }
    }

    fn current_day_tag() -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // Format day boundary (days since epoch)
        format!("day-{}", now / 86400)
    }

    pub fn maybe_reset_day(&mut self) {
        let current_tag = Self::current_day_tag();
        if self.day_tag != current_tag {
            self.sent_today = 0;
            self.current_burst_count = 0;
            self.day_tag = current_tag;
        }
    }
}

/// Anti-Ban Throttler enforcing safe delays, daily caps, and burst protections.
#[derive(Clone)]
pub struct AntiBanThrottler {
    config: ThrottlingConfig,
    stats: Arc<RwLock<HashMap<String, AccountStats>>>,
}

impl AntiBanThrottler {
    pub fn new(config: ThrottlingConfig) -> Self {
        Self {
            config,
            stats: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(ThrottlingConfig::default())
    }

    /// Checks if the account is allowed to send now.
    pub fn can_send(&self, account_key: &str) -> Result<(), String> {
        let mut stats_guard = self
            .stats
            .write()
            .map_err(|e| format!("Lock error in throttler: {}", e))?;

        let stats = stats_guard
            .entry(account_key.to_string())
            .or_insert_with(|| AccountStats::new(account_key));

        stats.maybe_reset_day();

        // 1. Check daily quota
        if stats.sent_today >= self.config.daily_quota {
            return Err(format!(
                "Account '{}' has reached daily quota limit ({} / {} messages)",
                account_key, stats.sent_today, self.config.daily_quota
            ));
        }

        // 2. Check burst cooldown
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if stats.current_burst_count >= self.config.burst_limit {
            let elapsed_since_burst = now.saturating_sub(stats.last_burst_epoch_secs);
            if elapsed_since_burst < self.config.cooldown_secs {
                let remaining = self.config.cooldown_secs - elapsed_since_burst;
                return Err(format!(
                    "Account '{}' is in burst cooldown. {}s remaining",
                    account_key, remaining
                ));
            } else {
                // Cooldown elapsed, reset burst counter
                stats.current_burst_count = 0;
            }
        }

        Ok(())
    }

    /// Records a dispatched message for the account.
    pub fn record_sent(&self, account_key: &str) {
        if let Ok(mut stats_guard) = self.stats.write() {
            let stats = stats_guard
                .entry(account_key.to_string())
                .or_insert_with(|| AccountStats::new(account_key));

            stats.maybe_reset_day();
            stats.sent_today += 1;
            stats.current_burst_count += 1;

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            stats.last_sent_epoch_secs = now;
            if stats.current_burst_count >= self.config.burst_limit {
                stats.last_burst_epoch_secs = now;
            }
        }
    }

    /// Calculates safe randomized delay before dispatching next message.
    pub fn calculate_delay(&self, account_key: &str) -> Duration {
        let min_s = self.config.min_delay_secs;
        let max_s = self.config.max_delay_secs.max(min_s);

        // Simple deterministic pseudo-random jitter without heavy rand dependency
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos() as u64;

        let delta = if max_s > min_s { max_s - min_s } else { 1 };
        let jitter = (now_nanos % (delta + 1)) + min_s;

        // If approaching burst limit, add minor backoff
        if let Ok(stats_guard) = self.stats.read() {
            if let Some(stats) = stats_guard.get(account_key) {
                if stats.current_burst_count >= self.config.burst_limit.saturating_sub(2) {
                    return Duration::from_secs(jitter + 5);
                }
            }
        }

        Duration::from_secs(jitter)
    }

    pub fn get_account_stats(&self, account_key: &str) -> Option<AccountStats> {
        let stats_guard = self.stats.read().ok()?;
        stats_guard.get(account_key).cloned()
    }

    pub fn reset_all_quotas(&self) {
        if let Ok(mut stats_guard) = self.stats.write() {
            for stats in stats_guard.values_mut() {
                stats.sent_today = 0;
                stats.current_burst_count = 0;
            }
        }
    }
}

/// Strategy for selecting next available account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RotationStrategy {
    RoundRobin,
    LeastRecentlyUsed,
    QuotaWeighted,
}

/// Manages a pool of active accounts per channel and selects healthy candidates.
#[derive(Clone)]
pub struct AccountRotator {
    strategy: RotationStrategy,
    accounts_by_channel: Arc<RwLock<HashMap<String, Vec<String>>>>,
    cursor_by_channel: Arc<RwLock<HashMap<String, usize>>>,
}

impl AccountRotator {
    pub fn new(strategy: RotationStrategy) -> Self {
        Self {
            strategy,
            accounts_by_channel: Arc::new(RwLock::new(HashMap::new())),
            cursor_by_channel: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn with_round_robin() -> Self {
        Self::new(RotationStrategy::RoundRobin)
    }

    /// Registers a single account into a channel's pool.
    pub fn register_account(&self, channel: &str, account_id: &str) {
        let ch = channel.trim().to_lowercase();
        let acc = account_id.trim().to_string();
        if acc.is_empty() {
            return;
        }

        if let Ok(mut map) = self.accounts_by_channel.write() {
            let list = map.entry(ch).or_default();
            if !list.contains(&acc) {
                list.push(acc);
            }
        }
    }

    /// Registers multiple accounts into a channel's pool.
    pub fn register_accounts(&self, channel: &str, account_ids: Vec<String>) {
        for acc in account_ids {
            self.register_account(channel, &acc);
        }
    }

    /// Removes an account from a channel's pool.
    pub fn remove_account(&self, channel: &str, account_id: &str) {
        let ch = channel.trim().to_lowercase();
        if let Ok(mut map) = self.accounts_by_channel.write() {
            if let Some(list) = map.get_mut(&ch) {
                list.retain(|a| a != account_id);
            }
        }
    }

    /// Retrieves all registered accounts for a channel.
    pub fn get_accounts(&self, channel: &str) -> Vec<String> {
        let ch = channel.trim().to_lowercase();
        let map = match self.accounts_by_channel.read() {
            Ok(m) => m,
            Err(_) => return Vec::new(),
        };
        map.get(&ch).cloned().unwrap_or_default()
    }

    /// Selects the next available account for the channel that passes throttler checks.
    pub fn next_account(&self, channel: &str, throttler: &AntiBanThrottler) -> Option<String> {
        let ch = channel.trim().to_lowercase();
        let accounts = self.get_accounts(&ch);
        if accounts.is_empty() {
            return None;
        }

        match self.strategy {
            RotationStrategy::RoundRobin => {
                let mut cursor_guard = self.cursor_by_channel.write().ok()?;
                let cursor = cursor_guard.entry(ch.clone()).or_insert(0);
                let total = accounts.len();

                for i in 0..total {
                    let idx = (*cursor + i) % total;
                    let candidate = &accounts[idx];
                    if throttler.can_send(candidate).is_ok() {
                        *cursor = (idx + 1) % total;
                        return Some(candidate.clone());
                    }
                }

                // If all candidates are throttled, return the current round-robin candidate as fallback
                let candidate = accounts[*cursor % total].clone();
                *cursor = (*cursor + 1) % total;
                Some(candidate)
            }
            RotationStrategy::LeastRecentlyUsed => {
                let mut best_account = None;
                let mut oldest_time = u64::MAX;

                for acc in &accounts {
                    if throttler.can_send(acc).is_ok() {
                        let last_sent = throttler
                            .get_account_stats(acc)
                            .map(|s| s.last_sent_epoch_secs)
                            .unwrap_or(0);
                        if last_sent < oldest_time {
                            oldest_time = last_sent;
                            best_account = Some(acc.clone());
                        }
                    }
                }

                best_account.or_else(|| accounts.first().cloned())
            }
            RotationStrategy::QuotaWeighted => {
                let mut best_account = None;
                let mut lowest_sent = u32::MAX;

                for acc in &accounts {
                    if throttler.can_send(acc).is_ok() {
                        let sent = throttler
                            .get_account_stats(acc)
                            .map(|s| s.sent_today)
                            .unwrap_or(0);
                        if sent < lowest_sent {
                            lowest_sent = sent;
                            best_account = Some(acc.clone());
                        }
                    }
                }

                best_account.or_else(|| accounts.first().cloned())
            }
        }
    }
}
