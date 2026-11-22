//! Synthetic rows only.

use super::*;
use crate::conversation::types::ChannelBadgeSummary;

fn item(key: &str, title: &str, at: Option<&str>, unread: u64, channel: Channel) -> InboxItem {
    InboxItem {
        key: key.to_string(),
        kind: if key.starts_with("lead:") {
            InboxItemKind::Customer
        } else {
            InboxItemKind::Unknown
        },
        lead_id: None,
        conversation_id: None,
        identity_id: None,
        title: title.to_string(),
        avatar_url: None,
        last_message_preview: Some(format!("hello from {title}")),
        last_message_at: at.map(str::to_string),
        unread_count: unread,
        channels: vec![ChannelBadgeSummary {
            channel,
            message_count: 1,
            unread_count: unread,
        }],
        reply_targets: Vec::new(),
        participant_count: 1,
        participants: Vec::new(),
    }
}

/// 25 rows, many sharing the same timestamp and unread count.
fn fixture() -> Vec<InboxItem> {
    (0..25)
        .map(|i| {
            item(
                &format!("lead:{i}"),
                &format!("Contact {}", i % 6),
                if i % 7 == 0 {
                    None
                } else {
                    Some(["2026-01-03", "2026-01-02", "2026-01-01"][i % 3])
                },
                (i % 3) as u64,
                if i % 2 == 0 {
                    Channel::Zalo
                } else {
                    Channel::WhatsApp
                },
            )
        })
        .collect()
}

fn walk(items: &[InboxItem], query: &InboxQuery, limit: u32) -> Vec<String> {
    let mut keys = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let page = page_inbox(items.to_vec(), query, cursor.as_deref(), limit).expect("page");
        keys.extend(page.items.iter().map(|i| i.key.clone()));
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => return keys,
        }
    }
}

#[test]
fn every_sort_and_direction_walks_each_row_once_in_the_unpaged_order() {
    let items = fixture();
    for sort in [InboxSort::Recent, InboxSort::Unread, InboxSort::Title] {
        for reverse in [false, true] {
            let query = InboxQuery {
                sort,
                reverse,
                ..InboxQuery::default()
            };
            let whole = page_inbox(items.clone(), &query, None, 200).expect("whole");
            let expected: Vec<_> = whole.items.iter().map(|i| i.key.clone()).collect();
            assert_eq!(expected.len(), 25);
            assert_eq!(
                walk(&items, &query, 4),
                expected,
                "{sort:?} reverse={reverse}"
            );
        }
    }
}

#[test]
fn recent_is_newest_first_with_undated_rows_last() {
    let items = fixture();
    let page = page_inbox(items, &InboxQuery::default(), None, 200).expect("page");
    let times: Vec<_> = page
        .items
        .iter()
        .map(|i| i.last_message_at.clone().unwrap_or_default())
        .collect();
    assert!(times.windows(2).all(|w| w[0] >= w[1]));
    assert!(times.last().is_some_and(String::is_empty));
}

#[test]
fn reverse_flips_the_order_exactly() {
    let items = fixture();
    let forward = walk(&items, &InboxQuery::default(), 6);
    let backward = walk(
        &items,
        &InboxQuery {
            reverse: true,
            ..InboxQuery::default()
        },
        6,
    );
    let mut flipped = forward.clone();
    flipped.reverse();
    assert_eq!(backward, flipped);
}

#[test]
fn filters_apply_before_paging_and_total_counts_them() {
    let items = fixture();
    let unread = InboxQuery {
        unread_only: true,
        ..InboxQuery::default()
    };
    let page = page_inbox(items.clone(), &unread, None, 5).expect("page");
    assert!(page.total > 5 && page.total < 25);
    assert_eq!(walk(&items, &unread, 5).len() as u64, page.total);

    let zalo = InboxQuery {
        channel: Some(Channel::Zalo),
        ..InboxQuery::default()
    };
    assert_eq!(
        page_inbox(items.clone(), &zalo, None, 200)
            .expect("zalo")
            .total,
        13
    );

    let search = InboxQuery {
        search: Some("CONTACT 3".into()),
        ..InboxQuery::default()
    };
    let found = page_inbox(items.clone(), &search, None, 200).expect("search");
    assert!(found.items.iter().all(|i| i.title == "Contact 3"));

    let by_preview = InboxQuery {
        search: Some("hello from contact 5".into()),
        ..InboxQuery::default()
    };
    assert!(
        page_inbox(items, &by_preview, None, 200)
            .expect("preview")
            .total
            > 0
    );
}

#[test]
fn kind_filter_separates_customers_from_unknown() {
    let mut items = fixture();
    items.push(item("identity:1", "Stranger", None, 0, Channel::Zalo));
    let unknown = InboxQuery {
        kind: Some(InboxItemKind::Unknown),
        ..InboxQuery::default()
    };
    let page = page_inbox(items, &unknown, None, 50).expect("page");
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].key, "identity:1");
}

#[test]
fn a_row_that_changes_while_scrolling_never_repeats() {
    let mut items = fixture();
    let query = InboxQuery::default();
    let first = page_inbox(items.clone(), &query, None, 5).expect("first");
    // A row from page 1 receives a new message and would jump to the very top.
    let moved = first.items[4].key.clone();
    for item in &mut items {
        if item.key == moved {
            item.last_message_at = Some("2026-02-01".into());
        }
    }
    let second = page_inbox(items, &query, first.next_cursor.as_deref(), 5).expect("second");
    assert!(second.items.iter().all(|i| i.key != moved));
    assert!(second
        .items
        .iter()
        .all(|i| first.items.iter().all(|seen| seen.key != i.key)));
}

#[test]
fn cursor_for_another_sort_or_garbage_is_rejected() {
    let items = fixture();
    let page = page_inbox(items.clone(), &InboxQuery::default(), None, 3).expect("page");
    let cursor = page.next_cursor.expect("more rows");
    let other = InboxQuery {
        sort: InboxSort::Title,
        ..InboxQuery::default()
    };
    assert!(matches!(
        page_inbox(items.clone(), &other, Some(&cursor), 3),
        Err(PageError::InvalidCursor(_))
    ));
    assert!(matches!(
        page_inbox(items, &InboxQuery::default(), Some("???"), 3),
        Err(PageError::InvalidCursor(_))
    ));
}

#[test]
fn empty_inbox_gives_an_empty_last_page() {
    let page = page_inbox(Vec::new(), &InboxQuery::default(), None, 10).expect("page");
    assert!(page.items.is_empty() && page.total == 0 && page.next_cursor.is_none());
}
