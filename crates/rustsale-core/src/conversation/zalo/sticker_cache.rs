use crate::conversation::zalo::client::ZaloStickerReference;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

const CACHE_TTL: Duration = Duration::from_secs(300);
const MAX_ENTRIES: usize = 100;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CacheKey {
    pub provider_account_key: String,
    pub query: String,
    pub limit: u32,
}

struct CacheEntry {
    stickers: Vec<ZaloStickerReference>,
    created_at: Instant,
}

pub struct ZaloStickerCache {
    entries: HashMap<CacheKey, CacheEntry>,
    access_order: VecDeque<CacheKey>,
}

impl Default for ZaloStickerCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ZaloStickerCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            access_order: VecDeque::new(),
        }
    }

    pub fn get(&mut self, key: &CacheKey, now: Instant) -> Option<Vec<ZaloStickerReference>> {
        if let Some(entry) = self.entries.get(key) {
            if now.duration_since(entry.created_at) < CACHE_TTL {
                return Some(entry.stickers.clone());
            }
        }
        self.entries.remove(key);
        None
    }

    pub fn insert(&mut self, key: CacheKey, stickers: Vec<ZaloStickerReference>, now: Instant) {
        if self.entries.len() >= MAX_ENTRIES && !self.entries.contains_key(&key) {
            if let Some(oldest_key) = self.access_order.pop_front() {
                self.entries.remove(&oldest_key);
            }
        }
        self.entries.insert(
            key.clone(),
            CacheEntry {
                stickers,
                created_at: now,
            },
        );
        self.access_order.retain(|k| k != &key);
        self.access_order.push_back(key);
    }
}
