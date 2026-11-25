//! Timeline paging order, as documented in `docs/v3/core-api.md`.

use super::commands_test::{direct_message, ServiceHarness};

/// The order v2 relies on: a page is oldest first, `next_cursor` is its oldest message and
/// is only present when the page is full; the next page holds the messages before it.
#[test]
fn a_page_is_oldest_first_and_the_cursor_is_its_oldest_message() {
    let harness = ServiceHarness::new();
    let mut conversation_id = 0;
    for (index, at) in ["10:00", "10:01", "10:02", "10:03", "10:04"]
        .iter()
        .enumerate()
    {
        let mut message = direct_message("imei-order", &format!("m-{index}"));
        message.provider_timestamp = Some(format!("2026-07-31T{at}:00.000Z"));
        conversation_id = harness
            .repository
            .apply_normalized_message(&message)
            .unwrap()
            .conversation_id;
    }

    let first = harness
        .service
        .get_conversation_timeline(conversation_id, None, 2)
        .unwrap();
    let texts: Vec<_> = first.items.iter().map(|m| m.occurred_at.clone()).collect();
    assert_eq!(
        texts,
        ["2026-07-31T10:03:00.000Z", "2026-07-31T10:04:00.000Z"]
    );
    let cursor = first.next_cursor.expect("a full page has a cursor");
    assert_eq!(cursor.message_id, first.items[0].id);
    assert_eq!(cursor.occurred_at, first.items[0].occurred_at);

    let second = harness
        .service
        .get_conversation_timeline(conversation_id, Some(&cursor), 2)
        .unwrap();
    assert_eq!(second.items[0].occurred_at, "2026-07-31T10:01:00.000Z");
    assert_eq!(second.items[1].occurred_at, "2026-07-31T10:02:00.000Z");

    let last = harness
        .service
        .get_conversation_timeline(conversation_id, second.next_cursor.as_ref(), 2)
        .unwrap();
    assert_eq!(last.items.len(), 1);
    assert_eq!(last.items[0].occurred_at, "2026-07-31T10:00:00.000Z");
    assert!(last.next_cursor.is_none());
}
