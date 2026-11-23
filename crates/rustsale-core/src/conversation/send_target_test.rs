//! Sending checks the target before anything is queued: a missing conversation or account is
//! `not_found`, an account of another conversation is refused, an account that is not connected
//! is `unavailable`. Nothing is written when a check fails.

use super::commands_test::{direct_message, ServiceHarness};
use super::error::SendTargetError;
use super::types::{AccountConnectionStatus, SendConversationMessageRequest};
use crate::db::open_db_connection;

struct Fixture {
    harness: ServiceHarness,
    conversation_id: i64,
    account_id: i64,
}

fn fixture(status: AccountConnectionStatus) -> Fixture {
    let harness = ServiceHarness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-send", "m-1"))
        .unwrap();
    let account_id = harness
        .repository
        .set_account_status("imei-send", status, None)
        .unwrap();
    Fixture {
        conversation_id: applied.conversation_id,
        account_id,
        harness,
    }
}

fn request(f: &Fixture, account: i64) -> SendConversationMessageRequest {
    SendConversationMessageRequest {
        conversation_id: f.conversation_id,
        channel_account_id: account,
        text: "hello".to_string(),
        reply_to_message_id: None,
    }
}

fn message_count(f: &Fixture) -> i64 {
    let conn = open_db_connection(f.harness.path.to_str().unwrap()).unwrap();
    conn.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
        .unwrap()
}

fn outbox_count(f: &Fixture) -> i64 {
    let conn = open_db_connection(f.harness.path.to_str().unwrap()).unwrap();
    conn.query_row("SELECT COUNT(*) FROM message_outbox_jobs", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn a_connected_account_passes() {
    let f = fixture(AccountConnectionStatus::Connected);
    assert_eq!(
        f.harness
            .service
            .check_send_target(&request(&f, f.account_id)),
        Ok(())
    );
}

#[test]
fn a_disconnected_account_is_unavailable_and_nothing_is_queued() {
    let f = fixture(AccountConnectionStatus::Disconnected);
    let before = (message_count(&f), outbox_count(&f));
    let error = f
        .harness
        .service
        .check_send_target(&request(&f, f.account_id))
        .unwrap_err();
    assert!(matches!(error, SendTargetError::AccountUnavailable { .. }));
    assert_eq!((message_count(&f), outbox_count(&f)), before);
}

#[test]
fn an_unknown_account_is_not_found_and_never_replaced() {
    let f = fixture(AccountConnectionStatus::Connected);
    let error = f
        .harness
        .service
        .check_send_target(&request(&f, 99_999_999))
        .unwrap_err();
    assert_eq!(error, SendTargetError::AccountNotFound(99_999_999));
}

#[test]
fn an_unknown_conversation_is_not_found() {
    let f = fixture(AccountConnectionStatus::Connected);
    let mut req = request(&f, f.account_id);
    req.conversation_id = 424_242;
    assert_eq!(
        f.harness.service.check_send_target(&req).unwrap_err(),
        SendTargetError::ConversationNotFound(424_242)
    );
}

#[test]
fn another_conversations_account_is_refused() {
    let f = fixture(AccountConnectionStatus::Connected);
    let other = f
        .harness
        .repository
        .set_account_status("imei-other", AccountConnectionStatus::Connected, None)
        .unwrap();
    assert_ne!(other, f.account_id);
    let error = f
        .harness
        .service
        .check_send_target(&request(&f, other))
        .unwrap_err();
    assert!(matches!(error, SendTargetError::AccountMismatch { .. }));
}
