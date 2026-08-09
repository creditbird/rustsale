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

pub struct ReactionEchoCache {
    entries: Mutex<HashMap<EchoKey, Instant>>,
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
            let expires_at = Instant::now() + Duration::from_secs(5);
            map.insert(key, expires_at);
            if map.len() > 200 {
                let now = Instant::now();
                map.retain(|_, exp| *exp > now);
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
            if let Some(exp) = map.remove(&key) {
                return exp > Instant::now();
            }
        }
        false
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
