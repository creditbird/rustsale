use super::migration::migrate;
use super::repository::{
    ConversationRepository, NormalizedGroupParticipant, NormalizedGroupSnapshot,
    NormalizedIdentity, NormalizedMessage, NormalizedReaction, NormalizedReceipt, NormalizedUndo,
    ReceiptKind,
};
use super::types::{Channel, ConversationKind, InboxItemKind, MessageDirection, MessageStatus};
use crate::db::open_db_connection;
use rusqlite::OptionalExtension;

struct Harness {
    path: std::path::PathBuf,
    repository: ConversationRepository,
}

impl Harness {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rustsale-conversation-repository-{}.db",
            rand::random::<u64>()
        ));
        let conn = open_db_connection(path.to_str().unwrap()).unwrap();
        conn.execute(
            "CREATE TABLE leads (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            [],
        )
        .unwrap();
        migrate(&conn).unwrap();
        drop(conn);
        Self {
            repository: ConversationRepository::new(path.to_string_lossy().to_string()),
            path,
        }
    }

    fn scalar(&self, sql: &str) -> i64 {
        let conn = open_db_connection(self.path.to_str().unwrap()).unwrap();
        conn.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    fn text(&self, sql: &str) -> String {
        let conn = open_db_connection(self.path.to_str().unwrap()).unwrap();
        conn.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    fn optional_text(&self, sql: &str) -> Option<String> {
        let conn = open_db_connection(self.path.to_str().unwrap()).unwrap();
        conn.query_row(sql, [], |row| row.get(0))
            .optional()
            .unwrap()
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(self.path.with_extension("db-wal"));
        let _ = std::fs::remove_file(self.path.with_extension("db-shm"));
    }
}

fn direct_message(account: &str, provider_message_id: &str) -> NormalizedMessage {
    NormalizedMessage {
        channel: Channel::Zalo,
        provider_account_key: account.to_string(),
        account_display_name: Some(account.to_string()),
        participant: Some(NormalizedIdentity {
            identity_namespace: "zalo_uid".to_string(),
            external_id: "peer-1".to_string(),
            display_name: Some("Peer One".to_string()),
            avatar_url: None,
            lead_id: None,
        }),
        sender_is_participant: true,
        external_thread_id: "peer-1".to_string(),
        conversation_kind: ConversationKind::Direct,
        conversation_title: Some("Peer One".to_string()),
        direction: MessageDirection::Inbound,
        provider_message_id: Some(provider_message_id.to_string()),
        provider_client_message_id: Some("client-1".to_string()),
        local_request_id: None,
        content_type: "text".to_string(),
        text_content: Some("hello".to_string()),
        content: serde_json::json!({"text": "hello"}),
        raw_payload: None,
        provider_timestamp: Some("2026-07-31T00:00:00.000Z".to_string()),
        ingested_at: "2026-07-31T00:00:01.000Z".to_string(),
        status: MessageStatus::Received,
    }
}

fn direct_message_for_peer(
    account: &str,
    provider_message_id: &str,
    peer_id: &str,
) -> NormalizedMessage {
    let mut message = direct_message(account, provider_message_id);
    let participant = message.participant.as_mut().expect("participant exists");
    participant.external_id = peer_id.to_string();
    participant.display_name = Some(peer_id.to_string());
    message.external_thread_id = peer_id.to_string();
    message.conversation_title = Some(peer_id.to_string());
    message.provider_client_message_id = Some(format!("client-{peer_id}"));
    message
}

#[test]
fn duplicate_provider_message_is_idempotent() {
    let harness = Harness::new();
    let first = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-1"))
        .unwrap();
    let second = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-1"))
        .unwrap();

    assert_eq!(first.message_id, second.message_id);
    assert!(!first.duplicate);
    assert!(second.duplicate);
}

#[test]
fn same_peer_on_two_zalo_accounts_creates_two_provider_conversations() {
    let harness = Harness::new();
    let first = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-1"))
        .unwrap();
    let second = harness
        .repository
        .apply_normalized_message(&direct_message("imei-b", "message-2"))
        .unwrap();

    assert_ne!(first.channel_account_id, second.channel_account_id);
    assert_ne!(first.conversation_id, second.conversation_id);
    assert_eq!(first.identity_id, second.identity_id);
}

#[test]
fn linking_identity_moves_direct_messages_into_contact_timeline_without_copying() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute("INSERT INTO leads (id, name) VALUES (7, 'Lead Seven')", [])
        .unwrap();
    drop(conn);
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-1"))
        .unwrap();

    harness
        .repository
        .link_identity(applied.identity_id.expect("direct message identity"), 7)
        .unwrap();
    let page = harness
        .repository
        .get_contact_timeline(7, &[Channel::Zalo], None, 50)
        .unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(
        page.items[0].provider_message_id.as_deref(),
        Some("message-1")
    );
}

#[test]
fn group_messages_are_excluded_from_contact_timeline() {
    let harness = Harness::new();
    let mut message = direct_message("imei-a", "group-message-1");
    message.conversation_kind = ConversationKind::Group;
    message.external_thread_id = "group-1".to_string();
    message
        .participant
        .as_mut()
        .expect("group sender identity")
        .lead_id = Some(7);
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute("INSERT INTO leads (id, name) VALUES (7, 'Lead Seven')", [])
        .unwrap();
    drop(conn);

    harness
        .repository
        .apply_normalized_message(&message)
        .unwrap();
    let page = harness
        .repository
        .get_contact_timeline(7, &[Channel::Zalo], None, 50)
        .unwrap();

    assert!(page.items.is_empty());
}

#[test]
fn timeline_pagination_returns_ascending_items_and_correct_next_cursor() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute("INSERT INTO leads (id, name) VALUES (7, 'Lead Seven')", [])
        .unwrap();
    drop(conn);

    for i in 1..=3 {
        let mut msg = direct_message("imei-a", &format!("msg-{i}"));
        msg.provider_timestamp = Some(format!("2026-07-31T00:00:0{i}.000Z"));
        msg.participant.as_mut().unwrap().lead_id = Some(7);
        harness.repository.apply_normalized_message(&msg).unwrap();
    }

    // Page 1: limit 2 -> returns newest 2 items, chronological ascending (msg-2, msg-3)
    let page1 = harness
        .repository
        .get_contact_timeline(7, &[Channel::Zalo], None, 2)
        .unwrap();
    assert_eq!(page1.items.len(), 2);
    assert_eq!(page1.items[0].provider_message_id.as_deref(), Some("msg-2"));
    assert_eq!(page1.items[1].provider_message_id.as_deref(), Some("msg-3"));
    let cursor = page1.next_cursor.expect("next cursor present");
    assert_eq!(cursor.message_id, page1.items[0].id);

    // Page 2: before cursor -> returns remaining item (msg-1)
    let page2 = harness
        .repository
        .get_contact_timeline(7, &[Channel::Zalo], Some(&cursor), 2)
        .unwrap();
    assert_eq!(page2.items.len(), 1);
    assert_eq!(page2.items[0].provider_message_id.as_deref(), Some("msg-1"));
    assert!(page2.next_cursor.is_none());
}

fn direct_message_with_client_id(
    provider_message_id: &str,
    client_message_id: &str,
) -> NormalizedMessage {
    let mut msg = direct_message("imei-a", provider_message_id);
    msg.provider_client_message_id = Some(client_message_id.to_string());
    msg
}

#[test]
fn reaction_upsert_targets_provider_or_client_message_id() {
    let harness = Harness::new();
    harness
        .repository
        .apply_normalized_message(&direct_message_with_client_id("message-1", "client-1"))
        .unwrap();
    let reaction = NormalizedReaction {
        provider_account_key: "imei-a".to_string(),
        external_thread_id: "peer-1".to_string(),
        is_group: false,
        provider_event_id: Some("reaction-event-1".to_string()),
        provider_message_id: "message-1".to_string(),
        provider_client_message_id: "client-1".to_string(),
        actor_external_id: "peer-1".to_string(),
        reaction: "heart".to_string(),
        removed: false,
        occurred_at: "2026-07-31T01:00:00.000Z".to_string(),
        raw_payload: serde_json::json!({}),
    };
    harness.repository.apply_reaction(&reaction).unwrap();
    harness.repository.apply_reaction(&reaction).unwrap();
    assert_eq!(harness.scalar("SELECT COUNT(*) FROM message_reactions"), 1);
}

#[test]
fn undo_marks_message_removed_without_deleting_it() {
    let harness = Harness::new();
    harness
        .repository
        .apply_normalized_message(&direct_message_with_client_id("message-1", "client-1"))
        .unwrap();
    harness
        .repository
        .apply_undo(&NormalizedUndo {
            provider_account_key: "imei-a".to_string(),
            external_thread_id: "peer-1".to_string(),
            is_group: false,
            provider_message_id: "message-1".to_string(),
            provider_client_message_id: "client-1".to_string(),
            occurred_at: "2026-07-31T01:01:00.000Z".to_string(),
            raw_payload: serde_json::json!({}),
        })
        .unwrap();
    assert_eq!(harness.scalar("SELECT COUNT(*) FROM messages"), 1);
    assert_eq!(
        harness.text("SELECT status FROM messages LIMIT 1"),
        "removed"
    );
    assert!(harness
        .optional_text("SELECT removed_at FROM messages LIMIT 1")
        .is_some());
}

#[test]
fn read_receipt_advances_outbound_message_status() {
    let harness = Harness::new();
    let mut message = direct_message_with_client_id("message-1", "client-1");
    message.direction = MessageDirection::Outbound;
    message.status = MessageStatus::Sent;
    harness
        .repository
        .apply_normalized_message(&message)
        .unwrap();
    harness
        .repository
        .apply_receipt(&NormalizedReceipt {
            provider_account_key: "imei-a".to_string(),
            external_thread_id: "peer-1".to_string(),
            is_group: false,
            provider_message_id: "message-1".to_string(),
            recipient_external_ids: vec!["peer-1".to_string()],
            kind: ReceiptKind::Read,
            occurred_at: "2026-07-31T01:02:00.000Z".to_string(),
            raw_payload: serde_json::json!({}),
        })
        .unwrap();
    assert_eq!(harness.text("SELECT status FROM messages LIMIT 1"), "read");
    assert_eq!(harness.scalar("SELECT COUNT(*) FROM message_receipts"), 1);
}

#[test]
fn checkpoint_upsert_is_monotonic_per_account_scope() {
    let harness = Harness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message_with_client_id("message-1", "client-1"))
        .unwrap();
    harness
        .repository
        .update_checkpoint(
            applied.channel_account_id,
            "direct_messages",
            Some("cursor-1"),
            Some("2026-07-31T01:00:00.000Z"),
            None,
        )
        .unwrap();
    harness
        .repository
        .update_checkpoint(
            applied.channel_account_id,
            "direct_messages",
            Some("cursor-2"),
            Some("2026-07-31T01:05:00.000Z"),
            None,
        )
        .unwrap();
    assert_eq!(
        harness.text("SELECT cursor FROM channel_sync_checkpoints LIMIT 1"),
        "cursor-2"
    );
}

#[test]
fn history_cursor_moves_backward_without_regressing_realtime_watermark() {
    let harness = Harness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message_with_client_id("message-1", "client-1"))
        .unwrap();

    harness
        .repository
        .update_checkpoint(
            applied.channel_account_id,
            "direct_messages",
            Some("message-newest"),
            Some("2026-07-31T10:00:00.000Z"),
            None,
        )
        .unwrap();
    harness
        .repository
        .update_history_cursor(
            applied.channel_account_id,
            "direct_message_history",
            "message-older-1",
        )
        .unwrap();
    harness
        .repository
        .update_history_cursor(
            applied.channel_account_id,
            "direct_message_history",
            "message-older-2",
        )
        .unwrap();

    assert_eq!(
        harness
            .repository
            .get_checkpoint(applied.channel_account_id, "direct_messages")
            .unwrap(),
        Some("message-newest".to_string())
    );
    assert_eq!(
        harness
            .repository
            .get_checkpoint(applied.channel_account_id, "direct_message_history")
            .unwrap(),
        Some("message-older-2".to_string())
    );
}

#[test]
fn group_snapshot_updates_title_avatar_and_participant_roles() {
    let harness = Harness::new();
    let conversation_id = harness
        .repository
        .apply_group_snapshot(&NormalizedGroupSnapshot {
            provider_account_key: "imei-a".to_string(),
            external_thread_id: "group-1".to_string(),
            title: "Project Alpha".to_string(),
            avatar_url: Some("https://example.invalid/group.png".to_string()),
            participants: vec![NormalizedGroupParticipant {
                identity: NormalizedIdentity {
                    identity_namespace: "zalo_uid".to_string(),
                    external_id: "member-1".to_string(),
                    display_name: Some("Member One".to_string()),
                    avatar_url: None,
                    lead_id: None,
                },
                participant_role: "admin".to_string(),
            }],
            membership_complete: true,
            occurred_at: "2026-07-31T01:05:00.000Z".to_string(),
        })
        .unwrap();
    assert!(conversation_id > 0);
    assert_eq!(
        harness.text("SELECT title FROM channel_conversations WHERE id = 1"),
        "Project Alpha"
    );
    assert_eq!(
        harness.text("SELECT participant_role FROM conversation_participants LIMIT 1"),
        "admin"
    );
}

#[test]
fn group_inbox_excludes_participants_who_left() {
    let harness = Harness::new();
    let conversation_id = harness
        .repository
        .apply_group_snapshot(&NormalizedGroupSnapshot {
            provider_account_key: "imei-a".to_string(),
            external_thread_id: "group-2".to_string(),
            title: "Project Beta".to_string(),
            avatar_url: None,
            participants: vec![
                NormalizedGroupParticipant {
                    identity: NormalizedIdentity {
                        identity_namespace: "zalo_uid".to_string(),
                        external_id: "member-active".to_string(),
                        display_name: Some("Active Member".to_string()),
                        avatar_url: None,
                        lead_id: None,
                    },
                    participant_role: "member".to_string(),
                },
                NormalizedGroupParticipant {
                    identity: NormalizedIdentity {
                        identity_namespace: "zalo_uid".to_string(),
                        external_id: "member-left".to_string(),
                        display_name: Some("Left Member".to_string()),
                        avatar_url: None,
                        lead_id: None,
                    },
                    participant_role: "member".to_string(),
                },
            ],
            membership_complete: true,
            occurred_at: "2026-07-31T01:05:00.000Z".to_string(),
        })
        .unwrap();

    // Mark member-left as having left (left_at IS NOT NULL)
    {
        let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
        conn.execute(
            "UPDATE conversation_participants SET left_at = '2026-07-31T02:00:00.000Z' WHERE conversation_id = ?1 AND identity_id = (SELECT id FROM channel_identities WHERE external_id = 'member-left')",
            [conversation_id],
        )
        .unwrap();
    }

    let items = harness.repository.list_inbox().unwrap();
    let group_item = items
        .iter()
        .find(|i| i.key == format!("conversation:{}", conversation_id))
        .unwrap();

    assert_eq!(group_item.participant_count, 1);
    assert_eq!(group_item.participants.len(), 1);
    assert_eq!(
        group_item.participants[0].display_name.as_deref(),
        Some("Active Member")
    );
}

#[test]
fn unlink_identity_resets_lead_id_and_returns_conversation_id() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute("INSERT INTO leads (id, name) VALUES (42, 'Lead 42')", [])
        .unwrap();
    drop(conn);

    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-1"))
        .unwrap();
    let identity_id = applied.identity_id.expect("identity exists");
    harness.repository.link_identity(identity_id, 42).unwrap();

    let inbox_before = harness.repository.list_inbox().unwrap();
    let customer_item = inbox_before.iter().find(|i| i.lead_id == Some(42)).unwrap();
    assert_eq!(customer_item.identity_id, None);

    let conv_id = harness.repository.unlink_identity(identity_id).unwrap();
    assert!(conv_id > 0);

    // Direct DB assertions
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let (db_lead_id, link_source, link_confidence, verified_at): (
        Option<i64>,
        String,
        f64,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT lead_id, link_source, link_confidence, verified_at FROM channel_identities WHERE id = ?1",
            [identity_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(db_lead_id, None);
    assert_eq!(link_source, "unlinked");
    assert_eq!(link_confidence, 0.0);
    assert_eq!(verified_at, None);

    let inbox = harness.repository.list_inbox().unwrap();
    let item = inbox
        .iter()
        .find(|i| i.identity_id == Some(identity_id))
        .unwrap();
    assert_eq!(item.kind, InboxItemKind::Unknown);
    assert_eq!(item.lead_id, None);
}

#[test]
fn unlink_lead_resets_lead_id_and_returns_conversation_id() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute("INSERT INTO leads (id, name) VALUES (42, 'Lead 42')", [])
        .unwrap();
    drop(conn);

    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-1"))
        .unwrap();
    let identity_id = applied.identity_id.expect("identity exists");
    harness.repository.link_identity(identity_id, 42).unwrap();

    let conv_id = harness.repository.unlink_lead(42).unwrap();
    assert_eq!(conv_id, applied.conversation_id);

    // Direct DB assertions
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let (db_lead_id, link_source, link_confidence, verified_at): (
        Option<i64>,
        String,
        f64,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT lead_id, link_source, link_confidence, verified_at FROM channel_identities WHERE id = ?1",
            [identity_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(db_lead_id, None);
    assert_eq!(link_source, "unlinked");
    assert_eq!(link_confidence, 0.0);
    assert_eq!(verified_at, None);
}

#[test]
fn unlink_lead_resets_all_identities_for_aggregated_customer() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute("INSERT INTO leads (id, name) VALUES (42, 'Lead 42')", [])
        .unwrap();
    drop(conn);

    let first = harness
        .repository
        .apply_normalized_message(&direct_message_for_peer("imei-a", "message-1", "peer-1"))
        .unwrap();
    let second = harness
        .repository
        .apply_normalized_message(&direct_message_for_peer("imei-b", "message-2", "peer-2"))
        .unwrap();
    let first_identity_id = first.identity_id.expect("first identity exists");
    let second_identity_id = second.identity_id.expect("second identity exists");
    harness
        .repository
        .link_identity(first_identity_id, 42)
        .unwrap();
    harness
        .repository
        .link_identity(second_identity_id, 42)
        .unwrap();

    harness.repository.unlink_lead(42).unwrap();

    assert_eq!(
        harness.scalar("SELECT COUNT(*) FROM channel_identities WHERE lead_id IS NULL"),
        2
    );
    assert_eq!(
        harness.scalar(
            "SELECT COUNT(*) FROM channel_identities
             WHERE link_source = 'unlinked'
               AND link_confidence = 0.0
               AND verified_at IS NULL"
        ),
        2
    );
}

#[test]
fn relink_lead_moves_all_identities_to_target_lead() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO leads (id, name) VALUES (42, 'Lead 42'), (84, 'Lead 84')",
        [],
    )
    .unwrap();
    drop(conn);

    let first = harness
        .repository
        .apply_normalized_message(&direct_message_for_peer("imei-a", "message-1", "peer-1"))
        .unwrap();
    let second = harness
        .repository
        .apply_normalized_message(&direct_message_for_peer("imei-b", "message-2", "peer-2"))
        .unwrap();
    harness
        .repository
        .link_identity(first.identity_id.expect("first identity exists"), 42)
        .unwrap();
    harness
        .repository
        .link_identity(second.identity_id.expect("second identity exists"), 42)
        .unwrap();

    let conversation_id = harness.repository.relink_lead(42, 84).unwrap();

    assert!(conversation_id > 0);
    assert_eq!(
        harness.scalar("SELECT COUNT(*) FROM channel_identities WHERE lead_id = 42"),
        0
    );
    assert_eq!(
        harness.scalar("SELECT COUNT(*) FROM channel_identities WHERE lead_id = 84"),
        2
    );
    assert_eq!(
        harness.scalar(
            "SELECT COUNT(*) FROM channel_identities
             WHERE lead_id = 84
               AND link_source = 'manual'
               AND link_confidence = 1.0
               AND verified_at IS NOT NULL"
        ),
        2
    );
}
