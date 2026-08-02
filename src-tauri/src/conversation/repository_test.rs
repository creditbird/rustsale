use super::migration::migrate;
use super::repository::{
    ConversationRepository, NormalizedGroupParticipant, NormalizedGroupSnapshot,
    NormalizedIdentity, NormalizedMessage, NormalizedReaction, NormalizedReceipt, NormalizedUndo,
    ReceiptKind, StagedAttachmentRecord,
};
use super::types::{Channel, ConversationKind, InboxItemKind, MessageDirection, MessageStatus};
use crate::db::open_db_connection;
use rusqlite::{params, OptionalExtension};

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
            "CREATE TABLE leads (id INTEGER PRIMARY KEY, name TEXT NOT NULL, phone TEXT, zalo_uid TEXT)",
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
        provider_client_message_id: Some(format!("client-{provider_message_id}")),
        local_request_id: None,
        content_type: "text".to_string(),
        text_content: Some("hello".to_string()),
        content: serde_json::json!({"text": "hello"}),
        attachments: Vec::new(),
        reply: None,
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
fn identity_timeline_combines_direct_messages_across_zalo_accounts() {
    let harness = Harness::new();
    let first = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-1"))
        .unwrap();
    let mut second_message = direct_message("imei-b", "message-2");
    second_message.provider_timestamp = Some("2026-07-31T00:00:02.000Z".to_string());
    let second = harness
        .repository
        .apply_normalized_message(&second_message)
        .unwrap();

    let page = harness
        .repository
        .get_identity_timeline(
            first.identity_id.expect("direct message identity"),
            None,
            50,
        )
        .unwrap();

    assert_ne!(first.conversation_id, second.conversation_id);
    assert_eq!(page.items.len(), 2);
    assert_eq!(
        page.items
            .iter()
            .map(|message| message.provider_message_id.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("message-1"), Some("message-2")]
    );
}

#[test]
fn identity_timeline_prefers_inbound_payload_name_over_stale_identity_name() {
    let harness = Harness::new();
    let mut message = direct_message("imei-a", "message-1");
    message.participant.as_mut().unwrap().display_name = Some("Hello World".to_string());
    message.raw_payload = Some(serde_json::json!({"dName": "Hồ Ngọc Phương Thảo"}));
    let applied = harness
        .repository
        .apply_normalized_message(&message)
        .unwrap();

    let page = harness
        .repository
        .get_identity_timeline(
            applied.identity_id.expect("direct message identity"),
            None,
            50,
        )
        .unwrap();

    assert_eq!(
        page.items[0].sender_display_name.as_deref(),
        Some("Hồ Ngọc Phương Thảo")
    );
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

#[test]
fn list_inbox_uses_identity_display_name_and_includes_zero_message_channels() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    conn.execute(
        "INSERT INTO leads (id, name) VALUES (99, 'Hello World')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
         VALUES (800, 'zalo', 'imei-zalo-test', 'Shop Zalo', 'connected', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_identities (id, lead_id, channel, identity_namespace, external_id, display_name, created_at, updated_at)
         VALUES (900, 99, 'zalo', 'zalo_uid', 'zalo-user-123', 'Tuan Nguyen Zalo', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, kind, external_thread_id, title, avatar_url, created_at, updated_at)
         VALUES (500, 800, 'direct', 'thread-123', NULL, NULL, '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
         VALUES (500, 900, 'member', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();

    drop(conn);

    let items = harness.repository.list_inbox().unwrap();
    let customer_item = items
        .iter()
        .find(|i| i.lead_id == Some(99))
        .expect("customer item found");

    // Must fall back to identity display name because lead name is 'Hello World'
    assert_eq!(customer_item.title, "Tuan Nguyen Zalo");

    // Must include zalo channel even with 0 messages in messages table
    assert_eq!(customer_item.channels.len(), 1);
    assert_eq!(customer_item.channels[0].channel, Channel::Zalo);
    assert_eq!(customer_item.channels[0].message_count, 0);
}

#[test]
fn list_inbox_rejects_account_name_as_unknown_identity_title() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
         VALUES (801, 'zalo', 'imei-account-name', 'Hello World', 'connected', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_identities (id, channel, identity_namespace, external_id, display_name, created_at, updated_at)
         VALUES (901, 'zalo', 'zalo_uid', 'peer-with-reply', 'Hello World', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z'),
                (902, 'zalo', 'zalo_uid', 'peer-outbound-only', 'Hello World', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, kind, external_thread_id, created_at, updated_at)
         VALUES (501, 801, 'direct', 'peer-with-reply', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z'),
                (502, 801, 'direct', 'peer-outbound-only', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
         VALUES (501, 901, 'member', '2026-08-01T00:00:00.000Z'),
                (502, 902, 'member', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO messages (conversation_id, sender_identity_id, direction, provider_message_id, content_type, text_content, content_json, raw_payload_json, provider_timestamp, ingested_at, status)
         VALUES (501, 901, 'inbound', 'reply-1', 'text', 'Xin chào', '{\"text\":\"Xin chào\"}', '{\"dName\":\"Hồ Ngọc Phương Thảo\"}', '2026-08-01T03:14:08.495Z', '2026-08-01T03:14:08.587Z', 'received'),
                (502, NULL, 'outbound', 'sent-1', 'text', 'Xin chào', '{\"text\":\"Xin chào\"}', '{\"dName\":\"Hello World\"}', '2026-08-01T03:15:00.000Z', '2026-08-01T03:15:00.100Z', 'sent')",
        [],
    )
    .unwrap();
    drop(conn);

    let items = harness.repository.list_inbox().unwrap();
    let replied = items
        .iter()
        .find(|item| item.identity_id == Some(901))
        .expect("replied unknown item found");
    let outbound_only = items
        .iter()
        .find(|item| item.identity_id == Some(902))
        .expect("outbound-only unknown item found");

    assert_eq!(replied.title, "Hồ Ngọc Phương Thảo");
    assert_eq!(outbound_only.title, "Unknown (peer-outbound-only)");
}

#[test]
fn delete_conversation_and_start_direct_conversation_execute_without_sql_errors() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
         VALUES (808, 'zalo', 'imei-del', 'Sale Account', 'connected', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();
    drop(conn);

    let start_req = crate::conversation::types::StartDirectConversationRequest {
        channel_account_id: 808,
        phone_or_external_id: "0909999888".to_string(),
        display_name: Some("Test User".to_string()),
        initial_message: "Hello phone".to_string(),
    };

    let msg = harness
        .repository
        .start_direct_conversation(&start_req, "2026-08-01T00:00:00.000Z")
        .unwrap();

    assert_eq!(msg.text_content.as_deref(), Some("Hello phone"));

    let del_req = crate::conversation::types::DeleteConversationRequest {
        conversation_id: Some(msg.conversation_id),
        identity_id: None,
        lead_id: None,
    };

    harness.repository.delete_conversation(&del_req).unwrap();
}

#[test]
fn profile_enrichment_queries_and_updates_work() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
         VALUES (999, 'zalo', 'imei-sync', 'Sync Account', 'connected', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_identities (id, channel, identity_namespace, external_id, display_name, avatar_url, created_at, updated_at)
         VALUES (1001, 'zalo', 'zalo_uid', 'uid-1001', NULL, NULL, '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at)
         VALUES (2001, 999, 'uid-1001', 'direct', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id) VALUES (2001, 1001)",
        [],
    )
    .unwrap();
    drop(conn);

    let needing = harness
        .repository
        .get_zalo_identities_needing_profile_sync(999, 10)
        .unwrap();
    assert_eq!(needing.len(), 1);
    assert_eq!(needing[0], (1001, "uid-1001".to_string()));

    let updated = harness
        .repository
        .mark_identities_profile_synced(
            &[1001],
            &[crate::conversation::zalo::client::NormalizedProfile {
                external_id: "uid-1001".to_string(),
                display_name: Some("Nguyen Van A".to_string()),
                avatar_url: Some("https://example.com/avt.jpg".to_string()),
            }],
        )
        .unwrap();
    assert_eq!(updated, 1);

    let needing_after = harness
        .repository
        .get_zalo_identities_needing_profile_sync(999, 10)
        .unwrap();
    assert_eq!(needing_after.len(), 0);
}

#[test]
fn profile_enrichment_no_avatar_user_prevents_infinite_requery_loop() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
         VALUES (998, 'zalo', 'imei-no-avt', 'No Avt Account', 'connected', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_identities (id, channel, identity_namespace, external_id, display_name, avatar_url, created_at, updated_at)
         VALUES (1002, 'zalo', 'zalo_uid', 'uid-no-avt', NULL, NULL, '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at)
         VALUES (2002, 998, 'uid-no-avt', 'direct', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id) VALUES (2002, 1002)",
        [],
    )
    .unwrap();
    drop(conn);

    let needing = harness
        .repository
        .get_zalo_identities_needing_profile_sync(998, 10)
        .unwrap();
    assert_eq!(needing.len(), 1);

    // Profile sync returns display name but NO avatar URL (user has no Zalo avatar)
    harness
        .repository
        .mark_identities_profile_synced(
            &[1002],
            &[crate::conversation::zalo::client::NormalizedProfile {
                external_id: "uid-no-avt".to_string(),
                display_name: Some("User Without Avatar".to_string()),
                avatar_url: None,
            }],
        )
        .unwrap();

    // Must NOT be returned on subsequent ticker runs
    let needing_again = harness
        .repository
        .get_zalo_identities_needing_profile_sync(998, 10)
        .unwrap();
    assert_eq!(needing_again.len(), 0);
}

#[test]
fn profile_enrichment_exponential_backoff_sequence_and_cap_at_24h() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    conn.execute(
        "INSERT INTO channel_identities (id, channel, identity_namespace, external_id, display_name, avatar_url, created_at, updated_at)
         VALUES (1003, 'zalo', 'zalo_uid', 'uid-fail', NULL, NULL, '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();
    drop(conn);

    // Perform 10 consecutive failures
    for attempt in 1..=10 {
        harness
            .repository
            .mark_identity_profile_sync_failed(&[1003], &format!("Network error {attempt}"))
            .unwrap();

        let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
        let (attempts, error): (i64, Option<String>) = conn
            .query_row(
                "SELECT profile_sync_attempts, profile_sync_error FROM channel_identities WHERE id = 1003",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(attempts, attempt as i64);
        assert_eq!(
            error.as_deref(),
            Some(format!("Network error {attempt}").as_str())
        );
    }
}

#[test]
fn profile_enrichment_7_day_ttl_refreshes_complete_profiles() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    let old_date = (chrono::Utc::now() - chrono::Duration::days(10)).to_rfc3339();
    let recent_date = (chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
         VALUES (997, 'zalo', 'imei-ttl', 'TTL Account', 'connected', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();

    // Identity 1004: Complete profile, synced 10 days ago (older than 7-day TTL)
    conn.execute(
        "INSERT INTO channel_identities (id, channel, identity_namespace, external_id, display_name, avatar_url, profile_synced_at, created_at, updated_at)
         VALUES (1004, 'zalo', 'zalo_uid', 'uid-old-ttl', 'Name 1', 'https://example.com/1.jpg', ?1, '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        params![old_date],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at)
         VALUES (2004, 997, 'uid-old-ttl', 'direct', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id) VALUES (2004, 1004)",
        [],
    )
    .unwrap();

    // Identity 1005: Complete profile, synced 2 hours ago (within 7-day TTL)
    conn.execute(
        "INSERT INTO channel_identities (id, channel, identity_namespace, external_id, display_name, avatar_url, profile_synced_at, created_at, updated_at)
         VALUES (1005, 'zalo', 'zalo_uid', 'uid-recent-ttl', 'Name 2', 'https://example.com/2.jpg', ?1, '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        params![recent_date],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at)
         VALUES (2005, 997, 'uid-recent-ttl', 'direct', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id) VALUES (2005, 1005)",
        [],
    )
    .unwrap();
    drop(conn);

    let needing = harness
        .repository
        .get_zalo_identities_needing_profile_sync(997, 10)
        .unwrap();
    assert_eq!(needing.len(), 1);
    assert_eq!(needing[0].0, 1004);
}

#[test]
fn start_direct_conversation_reuses_existing_thread_by_external_id() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
         VALUES (996, 'zalo', 'imei-existing', 'Account 996', 'connected', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();

    // Insert an existing channel_conversations with external_thread_id = '0949743369'
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, kind, external_thread_id, title, created_at, updated_at)
         VALUES (5001, 996, 'direct', '0949743369', '0949743369', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();
    drop(conn);

    let req = crate::conversation::types::StartDirectConversationRequest {
        channel_account_id: 996,
        phone_or_external_id: "0949743369".to_string(),
        display_name: Some("Anh Nam".to_string()),
        initial_message: "chao vo".to_string(),
    };

    // Calling start_direct_conversation must NOT fail with UNIQUE constraint error
    let msg = harness
        .repository
        .start_direct_conversation(&req, "2026-08-01T12:00:00.000Z")
        .unwrap();
    assert_eq!(msg.conversation_id, 5001);
}

#[test]
fn start_direct_conversation_reuses_existing_thread_by_lead_phone() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    conn.execute(
        "INSERT INTO leads (id, name, phone, zalo_uid)
         VALUES (700, 'Hồ Ngọc Phương Thảo', '0949743369', 'zalo-uid-thao')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
         VALUES (997, 'zalo', 'imei-thao', 'Account 997', 'connected', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_identities (id, lead_id, channel, identity_namespace, external_id, display_name, created_at, updated_at)
         VALUES (800, 700, 'zalo', 'zalo_uid', 'zalo-uid-thao', 'Hồ Ngọc Phương Thảo', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, kind, external_thread_id, title, created_at, updated_at)
         VALUES (6001, 997, 'direct', 'zalo-uid-thao', 'Hồ Ngọc Phương Thảo', '2026-08-01T00:00:00.000Z', '2026-08-01T00:00:00.000Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role)
         VALUES (6001, 800, 'member')",
        [],
    )
    .unwrap();

    drop(conn);

    let req = crate::conversation::types::StartDirectConversationRequest {
        channel_account_id: 997,
        phone_or_external_id: "0949743369".to_string(),
        display_name: Some("Hồ Ngọc Phương Thảo".to_string()),
        initial_message: "Xin chào Thảo".to_string(),
    };

    let msg = harness
        .repository
        .start_direct_conversation(&req, "2026-08-01T12:00:00.000Z")
        .unwrap();
    assert_eq!(msg.conversation_id, 6001);
}

#[test]
fn resolve_direct_phone_recipient_merges_provisional_thread_into_zalo_uid_thread() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    conn.execute(
        "INSERT INTO leads (id, name, phone, zalo_uid)
         VALUES (701, 'Thao', '0949743369', NULL)",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
         VALUES (998, 'zalo', 'imei-thao-live', 'Hello World', 'connected', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_identities (id, lead_id, channel, identity_namespace, external_id, display_name, link_source, created_at, updated_at)
         VALUES (801, NULL, 'zalo', 'zalo_uid', 'zalo-uid-thao-live', 'Hồ Ngọc Phương Thảo', 'unlinked', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, kind, external_thread_id, title, created_at, updated_at)
         VALUES (6002, 998, 'direct', 'zalo-uid-thao-live', 'Hồ Ngọc Phương Thảo', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role)
         VALUES (6002, 801, 'member')",
        [],
    )
    .unwrap();
    drop(conn);

    let request = crate::conversation::types::StartDirectConversationRequest {
        channel_account_id: 998,
        phone_or_external_id: "0949743369".to_string(),
        display_name: None,
        initial_message: "Xin chào Thảo".to_string(),
    };
    let message = harness
        .repository
        .start_direct_conversation(&request, "2026-08-02T00:45:54.120Z")
        .unwrap();
    assert_ne!(message.conversation_id, 6002);

    let resolved = harness
        .repository
        .resolve_direct_phone_recipient(
            message.id,
            998,
            "0949743369",
            "zalo-uid-thao-live",
            Some("Hồ Ngọc Phương Thảo"),
            Some("https://example.com/thao.jpg"),
            "2026-08-02T00:46:00.000Z",
        )
        .unwrap();

    assert_eq!(resolved.conversation_id, 6002);
    assert_eq!(resolved.external_thread_id, "zalo-uid-thao-live");

    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let message_conversation_id: i64 = conn
        .query_row(
            "SELECT conversation_id FROM messages WHERE id = ?1",
            [message.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(message_conversation_id, 6002);

    let lead_uid: String = conn
        .query_row("SELECT zalo_uid FROM leads WHERE id = 701", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(lead_uid, "zalo-uid-thao-live");

    let linked_lead_id: i64 = conn
        .query_row(
            "SELECT lead_id FROM channel_identities WHERE id = 801",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(linked_lead_id, 701);

    let provisional_conversation_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM channel_conversations WHERE external_thread_id = '0949743369'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(provisional_conversation_count, 0);
}

#[test]
fn enqueue_batch_creates_batch_parts_attachments_and_jobs_in_one_transaction() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    migrate(&conn).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at) VALUES (1, 'zalo', 'imei-a', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at) VALUES (1, 1, 'peer-1', 'direct', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();

    let db_path =
        std::env::temp_dir().join(format!("repo-batch-test-{}.db", rand::random::<u64>()));
    let conn_file = open_db_connection(db_path.to_str().unwrap()).unwrap();
    migrate(&conn_file).unwrap();
    conn_file.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at) VALUES (1, 'zalo', 'imei-a', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();
    conn_file.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at) VALUES (1, 1, 'peer-1', 'direct', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();

    let repo = ConversationRepository::new(db_path.to_str().unwrap().to_string());
    repo.insert_staged_attachment(&StagedAttachmentRecord {
        id: "rec-1".into(),
        conversation_id: 1,
        staged_attachment_id: "stage-1".into(),
        media_type: "image".into(),
        mime_type: "image/png".into(),
        file_name: "a.png".into(),
        file_size_bytes: 100,
        local_path: "/tmp/a.png".into(),
        thumbnail_path: None,
        width: Some(50),
        height: Some(50),
        duration_seconds: None,
        status: "staged".into(),
        expires_at: "2099-01-01T00:00:00Z".into(),
        created_at: "2026-08-01T00:00:00Z".into(),
        claimed_at: None,
    })
    .unwrap();

    repo.insert_staged_attachment(&StagedAttachmentRecord {
        id: "rec-2".into(),
        conversation_id: 1,
        staged_attachment_id: "stage-2".into(),
        media_type: "image".into(),
        mime_type: "image/png".into(),
        file_name: "b.png".into(),
        file_size_bytes: 200,
        local_path: "/tmp/b.png".into(),
        thumbnail_path: None,
        width: Some(60),
        height: Some(60),
        duration_seconds: None,
        status: "staged".into(),
        expires_at: "2099-01-01T00:00:00Z".into(),
        created_at: "2026-08-01T00:00:00Z".into(),
        claimed_at: None,
    })
    .unwrap();

    use crate::conversation::types::{
        ConversationSendPartContent, ConversationSendPartRequest, SendConversationBatchRequest,
    };

    let req = SendConversationBatchRequest {
        conversation_id: 1,
        client_batch_id: "batch-100".into(),
        reply_to_message_id: None,
        parts: vec![
            ConversationSendPartRequest {
                client_part_id: "part-1".into(),
                content: ConversationSendPartContent::Text {
                    text: "hello".into(),
                },
            },
            ConversationSendPartRequest {
                client_part_id: "part-2".into(),
                content: ConversationSendPartContent::Attachment {
                    staged_attachment_id: "stage-1".into(),
                },
            },
            ConversationSendPartRequest {
                client_part_id: "part-3".into(),
                content: ConversationSendPartContent::Attachment {
                    staged_attachment_id: "stage-2".into(),
                },
            },
        ],
    };

    let res = repo
        .enqueue_outbound_batch(&req, "2026-08-01T00:00:00Z")
        .unwrap();
    assert_eq!(res.client_batch_id, "batch-100");
    assert_eq!(res.messages.len(), 3);

    // Verify DB counts
    let conn_check = open_db_connection(db_path.to_str().unwrap()).unwrap();
    let batch_count: i64 = conn_check
        .query_row("SELECT COUNT(*) FROM message_send_batches", [], |r| {
            r.get(0)
        })
        .unwrap();
    let message_count: i64 = conn_check
        .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
        .unwrap();
    let attach_count: i64 = conn_check
        .query_row("SELECT COUNT(*) FROM message_attachments", [], |r| r.get(0))
        .unwrap();
    let job_count: i64 = conn_check
        .query_row("SELECT COUNT(*) FROM message_outbox_jobs", [], |r| r.get(0))
        .unwrap();

    assert_eq!(batch_count, 1);
    assert_eq!(message_count, 3);
    assert_eq!(attach_count, 2);
    assert_eq!(job_count, 3);

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn repeated_client_batch_id_returns_existing_batch_without_duplicates() {
    let db_path =
        std::env::temp_dir().join(format!("repo-dedup-test-{}.db", rand::random::<u64>()));
    let conn = open_db_connection(db_path.to_str().unwrap()).unwrap();
    migrate(&conn).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at) VALUES (1, 'zalo', 'imei-a', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at) VALUES (1, 1, 'peer-1', 'direct', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();

    let repo = ConversationRepository::new(db_path.to_str().unwrap().to_string());

    use crate::conversation::types::{
        ConversationSendPartContent, ConversationSendPartRequest, SendConversationBatchRequest,
    };

    let req = SendConversationBatchRequest {
        conversation_id: 1,
        client_batch_id: "batch-dedup".into(),
        reply_to_message_id: None,
        parts: vec![ConversationSendPartRequest {
            client_part_id: "part-1".into(),
            content: ConversationSendPartContent::Text {
                text: "hello".into(),
            },
        }],
    };

    let res1 = repo
        .enqueue_outbound_batch(&req, "2026-08-01T00:00:00Z")
        .unwrap();
    let res2 = repo
        .enqueue_outbound_batch(&req, "2026-08-01T00:00:00Z")
        .unwrap();

    assert_eq!(res1.batch_id, res2.batch_id);
    assert_eq!(res1.messages.len(), res2.messages.len());
    assert_eq!(res1.messages[0].id, res2.messages[0].id);

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn sticker_must_be_the_only_part_in_phase_one() {
    let db_path =
        std::env::temp_dir().join(format!("repo-sticker-test-{}.db", rand::random::<u64>()));
    let conn = open_db_connection(db_path.to_str().unwrap()).unwrap();
    migrate(&conn).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at) VALUES (1, 'zalo', 'imei-a', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at) VALUES (1, 1, 'peer-1', 'direct', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();

    let repo = ConversationRepository::new(db_path.to_str().unwrap().to_string());

    use crate::conversation::types::{
        ConversationSendPartContent, ConversationSendPartRequest, SendConversationBatchRequest,
    };

    let invalid_req = SendConversationBatchRequest {
        conversation_id: 1,
        client_batch_id: "batch-invalid-sticker".into(),
        reply_to_message_id: None,
        parts: vec![
            ConversationSendPartRequest {
                client_part_id: "part-1".into(),
                content: ConversationSendPartContent::Text {
                    text: "hello".into(),
                },
            },
            ConversationSendPartRequest {
                client_part_id: "part-2".into(),
                content: ConversationSendPartContent::Sticker {
                    sticker_id: 1,
                    cate_id: 2,
                    sticker_type: 1,
                },
            },
        ],
    };

    let err = repo
        .enqueue_outbound_batch(&invalid_req, "2026-08-01T00:00:00Z")
        .unwrap_err();
    assert_eq!(
        err.code,
        crate::conversation::error::ConversationErrorCode::InvalidRequest
    );

    let valid_sticker_req = SendConversationBatchRequest {
        conversation_id: 1,
        client_batch_id: "batch-valid-sticker".into(),
        reply_to_message_id: None,
        parts: vec![ConversationSendPartRequest {
            client_part_id: "part-1".into(),
            content: ConversationSendPartContent::Sticker {
                sticker_id: 1,
                cate_id: 2,
                sticker_type: 1,
            },
        }],
    };

    let res = repo
        .enqueue_outbound_batch(&valid_sticker_req, "2026-08-01T00:00:00Z")
        .unwrap();
    assert_eq!(res.messages.len(), 1);

    let _ = std::fs::remove_file(db_path);
}
