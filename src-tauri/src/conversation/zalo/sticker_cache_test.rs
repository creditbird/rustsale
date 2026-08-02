use super::sticker_cache::{CacheKey, ZaloStickerCache};
use crate::conversation::zalo::client::ZaloStickerReference;
use std::time::{Duration, Instant};

fn sample_stickers() -> Vec<ZaloStickerReference> {
    vec![ZaloStickerReference {
        sticker_id: 101,
        cate_id: 5,
        sticker_type: 1,
        url: "https://example.com/sticker1.webp".into(),
    }]
}

#[test]
fn same_account_query_and_limit_hits_cache_within_five_minutes() {
    let mut cache = ZaloStickerCache::new();
    let start = Instant::now();
    let key = CacheKey {
        provider_account_key: "acc-1".into(),
        query: "hello".into(),
        limit: 10,
    };

    cache.insert(key.clone(), sample_stickers(), start);

    let hit = cache.get(&key, start + Duration::from_secs(290));
    assert_eq!(hit, Some(sample_stickers()));
}

#[test]
fn different_accounts_do_not_share_sticker_cache() {
    let mut cache = ZaloStickerCache::new();
    let start = Instant::now();
    let key1 = CacheKey {
        provider_account_key: "acc-1".into(),
        query: "hello".into(),
        limit: 10,
    };
    let key2 = CacheKey {
        provider_account_key: "acc-2".into(),
        query: "hello".into(),
        limit: 10,
    };

    cache.insert(key1.clone(), sample_stickers(), start);

    assert_eq!(cache.get(&key1, start), Some(sample_stickers()));
    assert_eq!(cache.get(&key2, start), None);
}

#[test]
fn expired_entry_calls_provider_again() {
    let mut cache = ZaloStickerCache::new();
    let start = Instant::now();
    let key = CacheKey {
        provider_account_key: "acc-1".into(),
        query: "hello".into(),
        limit: 10,
    };

    cache.insert(key.clone(), sample_stickers(), start);

    let expired = cache.get(&key, start + Duration::from_secs(301));
    assert_eq!(expired, None);
}

#[test]
fn cache_evicts_oldest_entry_after_one_hundred_keys() {
    let mut cache = ZaloStickerCache::new();
    let start = Instant::now();

    for i in 0..105 {
        let key = CacheKey {
            provider_account_key: "acc-1".into(),
            query: format!("q{i}"),
            limit: 10,
        };
        cache.insert(key, sample_stickers(), start);
    }

    let oldest_key = CacheKey {
        provider_account_key: "acc-1".into(),
        query: "q0".into(),
        limit: 10,
    };
    let newest_key = CacheKey {
        provider_account_key: "acc-1".into(),
        query: "q104".into(),
        limit: 10,
    };

    assert_eq!(cache.get(&oldest_key, start), None);
    assert_eq!(cache.get(&newest_key, start), Some(sample_stickers()));
}
