//! Marking a conversation read tells every screen: it emits one `conversation-invalidated`
//! (reason `metadata`) per conversation whose unread count changed, and nothing when no row
//! changed.

use super::commands_test::{direct_message, ServiceHarness};
use super::events::{ConversationInvalidatedEvent, ConversationInvalidationReason};
use super::types::Channel;
use tokio::sync::broadcast::error::TryRecvError;

#[test]
fn mark_read_emits_an_invalidation_for_the_conversation() {
    let harness = ServiceHarness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-read", "m-read-1"))
        .unwrap();
    let mut events = harness.events.subscribe();
    // The apply above may have published its own events; only look at what follows.
    while events.try_recv().is_ok() {}

    let key = format!("conversation:{}", applied.conversation_id);
    let rows = harness.service.mark_conversation_read(&key).unwrap();
    assert_eq!(rows, 1);

    let event: ConversationInvalidatedEvent = events.try_recv().expect("an event after markRead");
    assert_eq!(event.conversation_id, applied.conversation_id);
    assert_eq!(event.channel, Channel::Zalo);
    assert_eq!(event.reason, ConversationInvalidationReason::Metadata);
    assert_eq!(event.message_id, None);
    assert!(matches!(events.try_recv(), Err(TryRecvError::Empty)));
}

#[test]
fn mark_read_with_nothing_unread_emits_nothing() {
    let harness = ServiceHarness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-read", "m-read-2"))
        .unwrap();
    let key = format!("conversation:{}", applied.conversation_id);
    harness.service.mark_conversation_read(&key).unwrap();

    let mut events = harness.events.subscribe();
    let rows = harness.service.mark_conversation_read(&key).unwrap();
    assert_eq!(rows, 0);
    assert!(matches!(events.try_recv(), Err(TryRecvError::Empty)));
}
