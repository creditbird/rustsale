use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
struct EchoKey {
    channel_account_id: i64,
    msg_id: String,
    reaction_code: String,
    actor_id: String,
}

#[derive(Clone, Copy, Debug)]
struct EchoExpectation {
    expires_at: Instant,
    remaining: u32,
}

pub struct ReactionEchoCache {
    entries: Mutex<HashMap<EchoKey, EchoExpectation>>,
}

impl Default for ReactionEchoCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ReactionEchoCache {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<ReactionEchoCache> = OnceLock::new();
        INSTANCE.get_or_init(ReactionEchoCache::new)
    }

    pub fn mark_expected(
        &self,
        channel_account_id: i64,
        msg_id: &str,
        reaction_code: &str,
        actor_id: &str,
    ) {
        let Some(key) = Self::key(channel_account_id, msg_id, reaction_code, actor_id) else {
            return;
        };
        if let Ok(mut map) = self.entries.lock() {
            let now = Instant::now();
            let expires_at = now + Duration::from_secs(5);
            map.entry(key)
                .and_modify(|expectation| {
                    expectation.remaining = if expectation.expires_at > now {
                        expectation.remaining.saturating_add(1)
                    } else {
                        1
                    };
                    expectation.expires_at = expires_at;
                })
                .or_insert(EchoExpectation {
                    expires_at,
                    remaining: 1,
                });
            if map.len() > 200 {
                map.retain(|_, expectation| expectation.expires_at > now);
            }
        }
    }

    pub fn consume_if_expected(
        &self,
        channel_account_id: i64,
        msg_id: &str,
        reaction_code: &str,
        actor_id: &str,
    ) -> bool {
        let Some(key) = Self::key(channel_account_id, msg_id, reaction_code, actor_id) else {
            return false;
        };
        if let Ok(mut map) = self.entries.lock() {
            let now = Instant::now();
            let mut remove = false;
            let matched = if let Some(expectation) = map.get_mut(&key) {
                if expectation.expires_at <= now {
                    remove = true;
                    false
                } else {
                    if expectation.remaining > 1 {
                        expectation.remaining -= 1;
                    } else {
                        remove = true;
                    }
                    true
                }
            } else {
                false
            };
            if remove {
                map.remove(&key);
            }
            return matched;
        }
        false
    }

    pub fn cancel_one_expected(
        &self,
        channel_account_id: i64,
        msg_id: &str,
        reaction_code: &str,
        actor_id: &str,
    ) {
        let Some(key) = Self::key(channel_account_id, msg_id, reaction_code, actor_id) else {
            return;
        };
        if let Ok(mut map) = self.entries.lock() {
            let remove = map.get_mut(&key).is_some_and(|expectation| {
                if expectation.expires_at <= Instant::now() || expectation.remaining <= 1 {
                    true
                } else {
                    expectation.remaining -= 1;
                    false
                }
            });
            if remove {
                map.remove(&key);
            }
        }
    }

    pub fn clear_expected(
        &self,
        channel_account_id: i64,
        msg_id: &str,
        reaction_code: &str,
        actor_id: &str,
    ) {
        let Some(key) = Self::key(channel_account_id, msg_id, reaction_code, actor_id) else {
            return;
        };
        if let Ok(mut map) = self.entries.lock() {
            map.remove(&key);
        }
    }

    fn key(
        channel_account_id: i64,
        msg_id: &str,
        reaction_code: &str,
        actor_id: &str,
    ) -> Option<EchoKey> {
        let msg_id = msg_id.trim();
        let actor_id = actor_id.trim();
        if msg_id.is_empty() || actor_id.is_empty() {
            return None;
        }
        Some(EchoKey {
            channel_account_id,
            msg_id: msg_id.to_string(),
            reaction_code: reaction_code.trim().to_lowercase(),
            actor_id: actor_id.to_string(),
        })
    }
}
