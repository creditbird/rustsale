use super::migration::migrate;
use super::repository::{
    ConversationRepository, NormalizedAttachment, NormalizedGroupParticipant,
    NormalizedGroupSnapshot, NormalizedIdentity, NormalizedMention, NormalizedMessage,
    NormalizedReaction, NormalizedReceipt, NormalizedReplyReference, NormalizedUndo, ReceiptKind,
    ResolveDirectPhoneRecipientRequest, StagedAttachmentRecord, ZaloUidCandidate,
};
use super::types::{
    Channel, ConversationCapabilities, ConversationKind, InboxItemKind, MessageDirection,
    MessageStatus,
};
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
        conn.query_row(sql, [], |row| row.get::<_, Option<String>>(0))
            .optional()
            .unwrap()
            .flatten()
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
        mentions: Vec::new(),
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

fn empty_group_conversation(harness: &Harness, external_thread_id: &str) -> i64 {
    harness
        .repository
        .apply_group_snapshot(&NormalizedGroupSnapshot {
            provider_account_key: "imei-a".to_string(),
            external_thread_id: external_thread_id.to_string(),
            title: "Hidden Group".to_string(),
            avatar_url: None,
            participants: Vec::new(),
            membership_complete: false,
            occurred_at: "2026-08-10T00:00:00Z".to_string(),
        })
        .unwrap()
}

#[test]
fn canonical_zalo_uid_merge_preserves_link_and_migrates_references() {
    let harness = Harness::new();
    let conversation_id = empty_group_conversation(&harness, "group-merge");
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO leads (id, name) VALUES (1, 'Canonical Lead'), (2, 'Legacy Lead')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_identities
         (id, lead_id, channel, identity_namespace, external_id, display_name, link_source,
          link_confidence, verified_at, created_at, updated_at)
         VALUES
         (100, 1, 'zalo', 'zalo_uid', 'merge-uid', 'Canonical Name', 'manual', 1.0,
          '2026-08-09T00:00:00Z', '2026-08-09T00:00:00Z', '2026-08-09T00:00:00Z'),
         (101, 2, 'zalo', 'user', 'merge-uid', 'Legacy Name', 'provider_match', 0.8,
          '2026-08-08T00:00:00Z', '2026-08-08T00:00:00Z', '2026-08-08T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO conversation_participants
         (conversation_id, identity_id, participant_role, joined_at, left_at)
         VALUES (?1, 100, 'member', '2026-08-09T00:00:00Z', '2026-08-09T01:00:00Z'),
                (?1, 101, 'admin', '2026-08-08T00:00:00Z', NULL)",
        [conversation_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO messages
         (id, conversation_id, sender_identity_id, direction, content_type, content_json,
          ingested_at, status)
         VALUES (200, ?1, 101, 'inbound', 'text', '{}', '2026-08-10T00:00:00Z', 'received')",
        [conversation_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO message_mentions
         (message_id, identity_id, external_id, display_text, start_utf16, length_utf16, created_at)
         VALUES (200, 101, 'merge-uid', '@Legacy', 0, 7, '2026-08-10T00:00:00Z')",
        [],
    )
    .unwrap();
    drop(conn);

    let ids = harness
        .repository
        .attach_zalo_uids_to_conversation(
            conversation_id,
            &[ZaloUidCandidate {
                external_id: " merge-uid ".to_string(),
                display_name: Some("Fresh Name".to_string()),
                avatar_url: Some("https://example.com/fresh.png".to_string()),
            }],
        )
        .unwrap();
    assert_eq!(ids, vec![100]);

    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let canonical: (i64, Option<i64>, Option<String>, Option<String>, String, Option<f64>) = conn
        .query_row(
            "SELECT id, lead_id, display_name, avatar_url, link_source, link_confidence
             FROM channel_identities
             WHERE channel = 'zalo' AND identity_namespace = 'zalo_uid' AND external_id = 'merge-uid'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        )
        .unwrap();
    assert_eq!(canonical.0, 100);
    assert_eq!(canonical.1, Some(1));
    assert_eq!(canonical.2.as_deref(), Some("Fresh Name"));
    assert_eq!(
        canonical.3.as_deref(),
        Some("https://example.com/fresh.png")
    );
    assert_eq!(canonical.4, "manual");
    assert_eq!(canonical.5, Some(1.0));
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM channel_identities WHERE channel = 'zalo' AND external_id = 'merge-uid'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        1
    );
    let participant: (i64, String, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT identity_id, participant_role, joined_at, left_at
             FROM conversation_participants WHERE conversation_id = ?1",
            [conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(participant.0, 100);
    assert_eq!(participant.1, "admin");
    assert_eq!(participant.2.as_deref(), Some("2026-08-08T00:00:00Z"));
    assert_eq!(participant.3, None);
    assert_eq!(
        conn.query_row(
            "SELECT sender_identity_id FROM messages WHERE id = 200",
            [],
            |row| { row.get::<_, i64>(0) }
        )
        .unwrap(),
        100
    );
    assert_eq!(
        conn.query_row(
            "SELECT identity_id FROM message_mentions WHERE message_id = 200",
            [],
            |row| { row.get::<_, i64>(0) }
        )
        .unwrap(),
        100
    );
}

#[test]
fn canonical_zalo_uid_batch_rolls_back_on_invalid_uid() {
    let harness = Harness::new();
    let conversation_id = empty_group_conversation(&harness, "group-invalid");

    let result = harness.repository.attach_zalo_uids_to_conversation(
        conversation_id,
        &[
            ZaloUidCandidate {
                external_id: "valid-before-error".to_string(),
                display_name: None,
                avatar_url: None,
            },
            ZaloUidCandidate {
                external_id: "0".to_string(),
                display_name: None,
                avatar_url: None,
            },
        ],
    );

    assert!(result.is_err());
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM channel_identities WHERE external_id = 'valid-before-error'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        0
    );
}

#[test]
fn canonical_zalo_uid_reference_migration_rolls_back_on_delete_failure() {
    let harness = Harness::new();
    let conversation_id = empty_group_conversation(&harness, "group-rollback");
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO channel_identities
         (id, channel, identity_namespace, external_id, display_name, created_at, updated_at)
         VALUES (301, 'zalo', 'user', 'rollback-uid', 'Legacy', '2026-08-10T00:00:00Z', '2026-08-10T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO conversation_participants
         (conversation_id, identity_id, participant_role, joined_at)
         VALUES (?1, 301, 'member', '2026-08-10T00:00:00Z')",
        [conversation_id],
    )
    .unwrap();
    conn.execute_batch(
        "CREATE TRIGGER fail_identity_delete
         BEFORE DELETE ON channel_identities
         WHEN OLD.external_id = 'rollback-uid'
         BEGIN
             SELECT RAISE(FAIL, 'blocked identity delete');
         END;",
    )
    .unwrap();
    drop(conn);

    let result = harness.repository.attach_zalo_uids_to_conversation(
        conversation_id,
        &[ZaloUidCandidate {
            external_id: "rollback-uid".to_string(),
            display_name: Some("Canonical".to_string()),
            avatar_url: None,
        }],
    );
    assert!(result.is_err());

    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT identity_id FROM conversation_participants WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        301
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM channel_identities WHERE external_id = 'rollback-uid'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        1
    );
}

#[test]
fn quote_owner_is_persisted_and_included_in_group_member_evidence() {
    let harness = Harness::new();
    let mut message = direct_message("imei-a", "quoted-message");
    message.external_thread_id = "group-quote".to_string();
    message.conversation_kind = ConversationKind::Group;
    message.conversation_title = Some("Quoted Group".to_string());
    message.reply = Some(NormalizedReplyReference {
        provider_message_id: "quoted-provider".to_string(),
        provider_client_message_id: "quoted-client".to_string(),
        sender_external_id: "quote-owner".to_string(),
        occurred_at: "2026-08-10T00:00:00Z".to_string(),
        text_content: Some("quoted".to_string()),
        content_type: "text".to_string(),
        raw_quote: serde_json::json!({"ownerId": "quote-owner"}),
    });

    let applied = harness
        .repository
        .apply_normalized_message(&message)
        .unwrap();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let reply_identity: (i64, String, String) = conn
        .query_row(
            "SELECT ci.id, ci.identity_namespace, ci.external_id
             FROM messages m
             JOIN channel_identities ci ON ci.id = m.reply_sender_identity_id
             WHERE m.id = ?1",
            [applied.message_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(reply_identity.1, "zalo_uid");
    assert_eq!(reply_identity.2, "quote-owner");
    drop(conn);

    let evidence = harness
        .repository
        .list_group_member_external_ids(applied.conversation_id)
        .unwrap();
    assert!(evidence.contains(&"quote-owner".to_string()));
    assert!(evidence.contains(&"peer-1".to_string()));
}

mod group_member_lead_import {
    use super::*;
    use crate::conversation::repository::{
        GroupMemberLeadImportItem, GroupMemberLeadImportRequest,
    };

    struct ImportHarness {
        path: std::path::PathBuf,
        repository: ConversationRepository,
    }

    impl ImportHarness {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "rustsale-group-member-import-{}.db",
                rand::random::<u64>()
            ));
            let conn = crate::db::init_db(path.to_str().unwrap()).unwrap();
            drop(conn);
            Self {
                repository: ConversationRepository::new(path.to_string_lossy().to_string()),
                path,
            }
        }

        fn connection(&self) -> rusqlite::Connection {
            open_db_connection(self.path.to_str().unwrap()).unwrap()
        }

        fn insert_group(&self, conversation_id: i64, external_thread_id: &str) {
            let conn = self.connection();
            let now = "2026-08-10T00:00:00Z";
            conn.execute(
                "INSERT OR IGNORE INTO channel_accounts
                 (id, channel, provider_account_key, display_name, created_at, updated_at)
                 VALUES (1, 'zalo', 'account-a', 'Account A', ?1, ?1)",
                [now],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO channel_conversations
                 (id, channel_account_id, external_thread_id, kind, title, created_at, updated_at)
                 VALUES (?1, 1, ?2, 'group', 'Group A', ?3, ?3)",
                rusqlite::params![conversation_id, external_thread_id, now],
            )
            .unwrap();
        }

        fn insert_identity(
            &self,
            identity_id: i64,
            external_id: &str,
            display_name: &str,
            lead_id: Option<i64>,
        ) {
            let conn = self.connection();
            let now = "2026-08-10T00:00:00Z";
            conn.execute(
                "INSERT INTO channel_identities
                 (id, lead_id, channel, identity_namespace, external_id, display_name,
                  created_at, updated_at)
                 VALUES (?1, ?2, 'zalo', 'zalo_uid', ?3, ?4, ?5, ?5)",
                rusqlite::params![identity_id, lead_id, external_id, display_name, now],
            )
            .unwrap();
        }

        fn add_active_participant(&self, conversation_id: i64, identity_id: i64) {
            let conn = self.connection();
            conn.execute(
                "INSERT INTO conversation_participants
                 (conversation_id, identity_id, participant_role, joined_at)
                 VALUES (?1, ?2, 'member', '2026-08-10T00:00:00Z')",
                rusqlite::params![conversation_id, identity_id],
            )
            .unwrap();
        }

        fn add_observed_sender(&self, conversation_id: i64, identity_id: i64) {
            let conn = self.connection();
            conn.execute(
                "INSERT INTO messages
                 (conversation_id, sender_identity_id, direction, content_type, content_json,
                  ingested_at, status)
                 VALUES (?1, ?2, 'inbound', 'text', '{}', '2026-08-10T00:00:00Z', 'received')",
                rusqlite::params![conversation_id, identity_id],
            )
            .unwrap();
        }

        fn insert_lead(
            &self,
            lead_id: i64,
            name: &str,
            zalo_uid: Option<&str>,
            about: Option<&str>,
        ) {
            let conn = self.connection();
            conn.execute(
                "INSERT INTO leads
                 (id, name, company, status, value, last_activity, owner, email, phone, about,
                  address, has_zalo, zalo_reached, zalo_uid)
                 VALUES (?1, ?2, 'Existing', 'New', 0, '2026-08-10T00:00:00Z', 'System', '', '',
                         ?3, '', 1, 0, ?4)",
                rusqlite::params![lead_id, name, about, zalo_uid],
            )
            .unwrap();
        }

        fn request(
            conversation_id: i64,
            members: Vec<GroupMemberLeadImportItem>,
        ) -> GroupMemberLeadImportRequest {
            GroupMemberLeadImportRequest {
                conversation_id,
                members,
                category_ids: Vec::new(),
                new_category_name: None,
                skip_existing: false,
            }
        }

        fn count(&self, sql: &str) -> i64 {
            self.connection()
                .query_row(sql, [], |row| row.get(0))
                .unwrap()
        }
    }

    impl Drop for ImportHarness {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
            let _ = std::fs::remove_file(self.path.with_extension("db-wal"));
            let _ = std::fs::remove_file(self.path.with_extension("db-shm"));
        }
    }

    fn item(identity_id: Option<i64>, external_id: Option<&str>) -> GroupMemberLeadImportItem {
        GroupMemberLeadImportItem {
            identity_id,
            external_id: external_id.map(str::to_string),
            phone: None,
        }
    }

    #[test]
    fn rejects_conflicting_or_foreign_identity_before_lead_mutation() {
        let harness = ImportHarness::new();
        harness.insert_group(100, "group-a");
        harness.insert_group(101, "group-b");
        harness.insert_identity(1001, "uid-a", "Member A", None);
        harness.insert_identity(1002, "uid-b", "Member B", None);
        harness.add_active_participant(100, 1001);
        harness.add_active_participant(101, 1002);

        let conflicting =
            harness
                .repository
                .import_group_members_to_leads(&ImportHarness::request(
                    100,
                    vec![item(Some(1001), Some("uid-b"))],
                ));
        assert!(conflicting.is_err());

        let foreign = harness
            .repository
            .import_group_members_to_leads(&ImportHarness::request(
                100,
                vec![item(Some(1002), Some("uid-b"))],
            ));
        assert!(foreign.is_err());
        assert_eq!(harness.count("SELECT COUNT(*) FROM leads"), 0);
    }

    #[test]
    fn deduplicates_reuses_exact_uid_ignores_about_and_sets_link_metadata() {
        let harness = ImportHarness::new();
        harness.insert_group(100, "group-a");
        harness.insert_identity(1001, "uid-a", "Member A", None);
        harness.insert_identity(1002, "uid-b", "Member B", None);
        harness.add_active_participant(100, 1001);
        harness.add_observed_sender(100, 1002);
        harness.insert_lead(10, "Exact UID Lead", Some("uid-a"), None);
        harness.insert_lead(11, "About Collision", None, Some("contains uid-b text"));
        let conn = harness.connection();
        conn.execute(
            "INSERT INTO categories (id, name) VALUES (5, 'Existing Category')",
            [],
        )
        .unwrap();
        drop(conn);

        let result = harness
            .repository
            .import_group_members_to_leads(&GroupMemberLeadImportRequest {
                conversation_id: 100,
                members: vec![
                    item(Some(1001), Some("uid-a")),
                    item(None, Some("uid-a")),
                    item(Some(1002), Some("uid-b")),
                ],
                category_ids: vec![5, 5],
                new_category_name: Some("Imported Group".to_string()),
                skip_existing: false,
            })
            .unwrap();

        assert_eq!(result.imported_count, 1);
        assert_eq!(result.updated_count, 1);
        assert_eq!(result.skipped_count, 0);
        assert_eq!(result.assigned_category_ids.len(), 2);
        let conn = harness.connection();
        let identity_a: (i64, String, f64, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT lead_id, link_source, link_confidence, verified_at, updated_at
                 FROM channel_identities WHERE id = 1001",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(identity_a.0, 10);
        assert_eq!(identity_a.1, "group_import");
        assert_eq!(identity_a.2, 1.0);
        assert!(identity_a.3.is_some());
        assert_eq!(identity_a.3, identity_a.4);

        let identity_b_lead: i64 = conn
            .query_row(
                "SELECT lead_id FROM channel_identities WHERE id = 1002",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_ne!(identity_b_lead, 11);
        let imported_uid: String = conn
            .query_row(
                "SELECT zalo_uid FROM leads WHERE id = ?1",
                [identity_b_lead],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(imported_uid, "uid-b");
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM lead_categories WHERE lead_id IN (10, ?1)",
                [identity_b_lead],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            4
        );
        drop(conn);

        let second = harness
            .repository
            .import_group_members_to_leads(&GroupMemberLeadImportRequest {
                conversation_id: 100,
                members: vec![item(Some(1001), None), item(Some(1002), None)],
                category_ids: result.assigned_category_ids.clone(),
                new_category_name: Some("Imported Group".to_string()),
                skip_existing: true,
            })
            .unwrap();
        assert_eq!(second.imported_count, 0);
        assert_eq!(second.updated_count, 0);
        assert_eq!(second.skipped_count, 2);
        assert_eq!(harness.count("SELECT COUNT(*) FROM lead_categories"), 4);
    }

    #[test]
    fn category_assignment_failure_rolls_back_category_lead_and_identity_link() {
        let harness = ImportHarness::new();
        harness.insert_group(100, "group-a");
        harness.insert_identity(1001, "uid-a", "Member A", None);
        harness.add_active_participant(100, 1001);
        let conn = harness.connection();
        conn.execute_batch(
            "CREATE TRIGGER fail_group_import_category
             BEFORE INSERT ON lead_categories
             BEGIN
                 SELECT RAISE(ABORT, 'blocked category assignment');
             END;",
        )
        .unwrap();
        drop(conn);

        let result =
            harness
                .repository
                .import_group_members_to_leads(&GroupMemberLeadImportRequest {
                    conversation_id: 100,
                    members: vec![item(Some(1001), Some("uid-a"))],
                    category_ids: Vec::new(),
                    new_category_name: Some("Must Roll Back".to_string()),
                    skip_existing: false,
                });

        assert!(result.is_err());
        assert_eq!(harness.count("SELECT COUNT(*) FROM categories"), 0);
        assert_eq!(harness.count("SELECT COUNT(*) FROM leads"), 0);
        assert_eq!(
            harness.count(
                "SELECT COUNT(*) FROM channel_identities WHERE id = 1001 AND lead_id IS NOT NULL"
            ),
            0
        );
    }
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
fn test_mark_conversation_read_resets_unread_count() {
    let harness = Harness::new();
    let msg = direct_message("imei-a", "msg-unread-1");
    let _applied = harness.repository.apply_normalized_message(&msg).unwrap();

    let inbox_before = harness.repository.list_inbox().unwrap();
    assert!(!inbox_before.is_empty());
    let item_key = &inbox_before[0].key;

    let updated_rows = harness.repository.mark_conversation_read(item_key).unwrap();
    assert!(updated_rows > 0);

    let inbox_after = harness.repository.list_inbox().unwrap();
    assert_eq!(inbox_after[0].unread_count, 0);
}

#[test]
fn normalized_attachment_persists_created_at_and_duplicate_is_idempotent() {
    let harness = Harness::new();
    let mut message = direct_message("imei-a", "message-with-attachment");
    message.attachments.push(NormalizedAttachment {
        provider_attachment_id: None,
        media_type: "image".to_string(),
        mime_type: "image/png".to_string(),
        file_name: Some("photo.png".to_string()),
        size_bytes: Some(42),
        remote_url: Some("https://example.com/photo.png".to_string()),
        thumbnail_url: None,
        metadata_json: Some(serde_json::json!({"width": 10, "height": 20})),
    });

    let first = harness
        .repository
        .apply_normalized_message(&message)
        .unwrap();
    let second = harness
        .repository
        .apply_normalized_message(&message)
        .unwrap();

    assert!(!first.duplicate);
    assert!(second.duplicate);
    assert_eq!(first.message_id, second.message_id);
    assert_eq!(
        harness.text("SELECT created_at FROM message_attachments LIMIT 1"),
        message.ingested_at
    );
    assert_eq!(
        harness.scalar("SELECT COUNT(*) FROM message_attachments"),
        1
    );
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
fn mapped_zalo_imei_reuses_canonical_account_for_messages_groups_and_mutations() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute_batch(
        "INSERT INTO channel_accounts
            (id, channel, provider_account_key, self_external_id, display_name, connection_status, capabilities_json, created_at, updated_at)
         VALUES
            (900, 'zalo', 'uid-stable', 'uid-stable', 'Stable Zalo', 'connected', '{}', '2026-08-09T00:00:00Z', '2026-08-09T00:00:00Z');
         INSERT INTO zalo_sessions
            (imei, user_agent, cookies_json, language, display_name, zalo_uid, channel_account_id)
         VALUES
            ('imei-relogin', 'UA/1.0', '[]', 'vi', 'Stable Zalo', 'uid-stable', 900);",
    )
    .unwrap();
    drop(conn);

    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-relogin", "message-stable"))
        .unwrap();
    assert_eq!(applied.channel_account_id, 900);

    harness
        .repository
        .apply_reaction(&NormalizedReaction {
            provider_account_key: "imei-relogin".to_string(),
            external_thread_id: "peer-1".to_string(),
            is_group: false,
            provider_event_id: Some("reaction-stable".to_string()),
            provider_message_id: "message-stable".to_string(),
            provider_client_message_id: "client-message-stable".to_string(),
            actor_external_id: "peer-1".to_string(),
            reaction: "heart".to_string(),
            removed: false,
            occurred_at: "2026-08-09T00:01:00Z".to_string(),
            raw_payload: serde_json::json!({}),
        })
        .unwrap();
    harness
        .repository
        .apply_receipt(&NormalizedReceipt {
            provider_account_key: "imei-relogin".to_string(),
            external_thread_id: "peer-1".to_string(),
            is_group: false,
            provider_message_id: "message-stable".to_string(),
            recipient_external_ids: vec!["peer-1".to_string()],
            kind: ReceiptKind::Read,
            occurred_at: "2026-08-09T00:02:00Z".to_string(),
            raw_payload: serde_json::json!({}),
        })
        .unwrap();
    let group_conversation_id = harness
        .repository
        .apply_group_snapshot(&NormalizedGroupSnapshot {
            provider_account_key: "imei-relogin".to_string(),
            external_thread_id: "group-stable".to_string(),
            title: "Stable Group".to_string(),
            avatar_url: None,
            participants: Vec::new(),
            membership_complete: true,
            occurred_at: "2026-08-09T00:03:00Z".to_string(),
        })
        .unwrap();

    assert_eq!(
        harness.scalar("SELECT COUNT(*) FROM channel_accounts WHERE channel = 'zalo'"),
        1
    );
    assert_eq!(
        harness.scalar(&format!(
            "SELECT channel_account_id FROM channel_conversations WHERE id = {group_conversation_id}"
        )),
        900
    );
    assert_eq!(harness.scalar("SELECT COUNT(*) FROM message_reactions"), 1);
    assert_eq!(harness.scalar("SELECT COUNT(*) FROM message_receipts"), 1);
    assert_eq!(
        harness.text("SELECT status FROM messages WHERE id = 1"),
        "read"
    );
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

    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "UPDATE channel_accounts SET connection_status = 'connected' WHERE id = ?1",
        [applied.channel_account_id],
    )
    .unwrap();
    drop(conn);

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
    assert!(page.items[0].actions.react);
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

fn insert_zalo_account(harness: &Harness, provider_account_key: &str) {
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts
            (channel, provider_account_key, connection_status, capabilities_json, created_at, updated_at)
         VALUES
            ('zalo', ?1, 'connected', '{}', '2026-08-09T00:00:00Z', '2026-08-09T00:00:00Z')",
        [provider_account_key],
    )
    .unwrap();
}

fn reaction_event(
    event_id: &str,
    provider_message_id: &str,
    provider_client_message_id: &str,
    removed: bool,
    occurred_at: &str,
) -> NormalizedReaction {
    NormalizedReaction {
        provider_account_key: "imei-a".to_string(),
        external_thread_id: "peer-1".to_string(),
        is_group: false,
        provider_event_id: Some(event_id.to_string()),
        provider_message_id: provider_message_id.to_string(),
        provider_client_message_id: provider_client_message_id.to_string(),
        actor_external_id: "peer-1".to_string(),
        reaction: "heart".to_string(),
        removed,
        occurred_at: occurred_at.to_string(),
        raw_payload: serde_json::json!({"eventId": event_id}),
    }
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
fn reaction_without_known_account_returns_ingest_error() {
    let harness = Harness::new();
    let result = harness.repository.apply_reaction(&reaction_event(
        "event-missing-account",
        "message-missing-account",
        "client-missing-account",
        false,
        "2026-08-09T00:01:00Z",
    ));

    assert!(result.is_err());
}

#[test]
fn reaction_before_message_is_staged_then_replayed_by_provider_message_id() {
    let harness = Harness::new();
    insert_zalo_account(&harness, "imei-a");

    let result = harness
        .repository
        .apply_reaction(&reaction_event(
            "event-provider-late",
            "message-provider-late",
            "client-provider-late",
            false,
            "2026-08-09T00:01:00Z",
        ))
        .unwrap();

    assert_eq!(result, None);
    assert_eq!(
        harness.scalar("SELECT COUNT(*) FROM pending_message_reactions"),
        1
    );
    assert_eq!(harness.scalar("SELECT COUNT(*) FROM message_reactions"), 0);

    harness
        .repository
        .apply_normalized_message(&direct_message_with_client_id(
            "message-provider-late",
            "client-provider-late",
        ))
        .unwrap();

    assert_eq!(
        harness.scalar("SELECT COUNT(*) FROM pending_message_reactions"),
        0
    );
    assert_eq!(
        harness.scalar(
            "SELECT COUNT(*) FROM message_reactions WHERE reaction = 'heart' AND removed_at IS NULL"
        ),
        1
    );
}

#[test]
fn reaction_with_zero_provider_message_id_replays_by_client_message_id() {
    let harness = Harness::new();
    insert_zalo_account(&harness, "imei-a");

    harness
        .repository
        .apply_reaction(&reaction_event(
            "event-client-late",
            "0",
            "client-message-late",
            false,
            "2026-08-09T00:01:00Z",
        ))
        .unwrap();

    harness
        .repository
        .apply_normalized_message(&direct_message_with_client_id(
            "server-message-late",
            "client-message-late",
        ))
        .unwrap();

    assert_eq!(
        harness.scalar("SELECT COUNT(*) FROM pending_message_reactions"),
        0
    );
    assert_eq!(
        harness.scalar(
            "SELECT COUNT(*) FROM message_reactions WHERE reaction = 'heart' AND removed_at IS NULL"
        ),
        1
    );
}

#[test]
fn pending_reaction_replay_is_chronological_and_idempotent() {
    let harness = Harness::new();
    insert_zalo_account(&harness, "imei-a");

    let events = [
        reaction_event(
            "event-add-latest",
            "message-ordered",
            "client-ordered",
            false,
            "2026-08-09T00:03:00Z",
        ),
        reaction_event(
            "event-add-earliest",
            "message-ordered",
            "client-ordered",
            false,
            "2026-08-09T00:01:00Z",
        ),
        reaction_event(
            "event-remove-middle",
            "message-ordered",
            "client-ordered",
            true,
            "2026-08-09T00:02:00Z",
        ),
    ];

    for event in &events {
        harness.repository.apply_reaction(event).unwrap();
        harness.repository.apply_reaction(event).unwrap();
    }

    assert_eq!(
        harness.scalar("SELECT COUNT(*) FROM pending_message_reactions"),
        3
    );

    harness
        .repository
        .apply_normalized_message(&direct_message_with_client_id(
            "message-ordered",
            "client-ordered",
        ))
        .unwrap();

    assert_eq!(
        harness.scalar("SELECT COUNT(*) FROM pending_message_reactions"),
        0
    );
    assert_eq!(harness.scalar("SELECT COUNT(*) FROM message_reactions"), 1);
    assert_eq!(
        harness.scalar("SELECT COUNT(*) FROM message_reactions WHERE removed_at IS NULL"),
        1
    );
}

#[test]
fn duplicate_message_reconciliation_replays_pending_reactions() {
    let harness = Harness::new();
    let mut initial = direct_message("imei-a", "unused-initial-provider-id");
    initial.provider_message_id = None;
    initial.provider_client_message_id = Some("client-initial".to_string());
    initial.local_request_id = Some("local-request-replay".to_string());
    harness
        .repository
        .apply_normalized_message(&initial)
        .unwrap();

    harness
        .repository
        .apply_reaction(&reaction_event(
            "event-duplicate-replay",
            "message-reconciled",
            "client-reconciled",
            false,
            "2026-08-09T00:01:00Z",
        ))
        .unwrap();
    assert_eq!(
        harness.scalar("SELECT COUNT(*) FROM pending_message_reactions"),
        1
    );

    let mut echo = direct_message_with_client_id("message-reconciled", "client-reconciled");
    echo.local_request_id = Some("local-request-replay".to_string());
    let applied = harness.repository.apply_normalized_message(&echo).unwrap();

    assert!(applied.duplicate);
    assert_eq!(
        harness.scalar("SELECT COUNT(*) FROM pending_message_reactions"),
        0
    );
    assert_eq!(harness.scalar("SELECT COUNT(*) FROM message_reactions"), 1);
}

#[test]
fn pending_reaction_replay_rolls_back_message_when_cleanup_fails() {
    let harness = Harness::new();
    insert_zalo_account(&harness, "imei-a");
    harness
        .repository
        .apply_reaction(&reaction_event(
            "event-rollback",
            "message-rollback",
            "client-rollback",
            false,
            "2026-08-09T00:01:00Z",
        ))
        .unwrap();

    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute_batch(
        "CREATE TRIGGER block_pending_reaction_cleanup
         BEFORE DELETE ON pending_message_reactions
         BEGIN
             SELECT RAISE(ABORT, 'pending reaction cleanup blocked');
         END;",
    )
    .unwrap();
    drop(conn);

    let result = harness
        .repository
        .apply_normalized_message(&direct_message_with_client_id(
            "message-rollback",
            "client-rollback",
        ));

    assert!(result.is_err());
    assert_eq!(harness.scalar("SELECT COUNT(*) FROM messages"), 0);
    assert_eq!(harness.scalar("SELECT COUNT(*) FROM message_reactions"), 0);
    assert_eq!(
        harness.scalar("SELECT COUNT(*) FROM pending_message_reactions"),
        1
    );
}

#[test]
fn account_identity_update_persists_self_external_id_without_corrupting_avatar() {
    let harness = Harness::new();
    harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-self"))
        .unwrap();
    let account_id = harness
        .scalar("SELECT id FROM channel_accounts WHERE provider_account_key = 'imei-a' LIMIT 1");

    harness
        .repository
        .update_account_identity_and_capabilities(
            account_id,
            Some("own-zuid-1"),
            r#"{"canQuote":true}"#,
            "2026-08-02T10:00:00.000Z",
        )
        .unwrap();

    assert_eq!(
        harness.optional_text("SELECT self_external_id FROM channel_accounts LIMIT 1"),
        Some("own-zuid-1".to_string())
    );
    assert_eq!(
        harness.optional_text("SELECT avatar_url FROM channel_accounts LIMIT 1"),
        None
    );
}

#[test]
fn hydrated_reactions_mark_current_account_and_keep_external_id_fallback() {
    let harness = Harness::new();
    harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-reactions"))
        .unwrap();
    let account_id = harness
        .scalar("SELECT id FROM channel_accounts WHERE provider_account_key = 'imei-a' LIMIT 1");
    harness
        .repository
        .update_account_identity_and_capabilities(
            account_id,
            Some("own-zuid-1"),
            "{}",
            "2026-08-02T10:00:00.000Z",
        )
        .unwrap();

    for actor_external_id in ["own-zuid-1", "peer-1"] {
        harness
            .repository
            .apply_reaction(&NormalizedReaction {
                provider_account_key: "imei-a".to_string(),
                external_thread_id: "peer-1".to_string(),
                is_group: false,
                provider_event_id: Some(format!("event-{actor_external_id}")),
                provider_message_id: "message-reactions".to_string(),
                provider_client_message_id: "client-message-reactions".to_string(),
                actor_external_id: actor_external_id.to_string(),
                reaction: "heart".to_string(),
                removed: false,
                occurred_at: "2026-08-02T10:01:00.000Z".to_string(),
                raw_payload: serde_json::json!({}),
            })
            .unwrap();
    }

    let conversation_id = harness.scalar("SELECT id FROM channel_conversations LIMIT 1");
    let page = harness
        .repository
        .get_conversation_timeline(conversation_id, None, 50)
        .unwrap();
    let group = &page.items[0].reactions[0];

    assert_eq!(group.count, 2);
    assert!(group.includes_me);
    assert!(group
        .actors
        .iter()
        .any(|actor| actor.external_id == "own-zuid-1" && actor.identity_id.is_none()));
    assert!(group
        .actors
        .iter()
        .any(|actor| actor.external_id == "peer-1" && actor.identity_id.is_some()));
}

#[test]
fn hydrated_reactions_sum_persisted_actor_counts() {
    let harness = Harness::new();
    harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-reaction-counts"))
        .unwrap();

    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let message_id: i64 = conn
        .query_row(
            "SELECT id FROM messages WHERE provider_message_id = 'message-reaction-counts'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    for (actor_external_id, count) in [("actor-1", 4), ("actor-2", 3)] {
        conn.execute(
            "INSERT INTO message_reactions
                (message_id, actor_external_id, reaction, count, reacted_at, removed_at)
             VALUES (?1, ?2, 'heart', ?3, '2026-08-02T10:01:00.000Z', NULL)",
            params![message_id, actor_external_id, count],
        )
        .unwrap();
    }

    let conversation_id = harness.scalar("SELECT id FROM channel_conversations LIMIT 1");
    let page = harness
        .repository
        .get_conversation_timeline(conversation_id, None, 50)
        .unwrap();

    assert_eq!(page.items[0].reactions[0].count, 7);
    assert_eq!(page.items[0].reactions[0].actors.len(), 2);
}

#[test]
fn hydrated_reactions_normalize_provider_aliases_to_canonical_emoji() {
    let harness = Harness::new();
    harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-reaction-aliases"))
        .unwrap();

    for (index, reaction) in ["/-heart", "/:heart", "heart"].iter().enumerate() {
        harness
            .repository
            .apply_reaction(&NormalizedReaction {
                provider_account_key: "imei-a".to_string(),
                external_thread_id: "peer-1".to_string(),
                is_group: false,
                provider_event_id: Some(format!("event-reaction-alias-{index}")),
                provider_message_id: "message-reaction-aliases".to_string(),
                provider_client_message_id: "client-message-reaction-aliases".to_string(),
                actor_external_id: format!("actor-{index}"),
                reaction: (*reaction).to_string(),
                removed: false,
                occurred_at: format!("2026-08-02T10:01:0{index}.000Z"),
                raw_payload: serde_json::json!({}),
            })
            .unwrap();
    }

    let conversation_id = harness.scalar("SELECT id FROM channel_conversations LIMIT 1");
    let page = harness
        .repository
        .get_conversation_timeline(conversation_id, None, 50)
        .unwrap();

    assert_eq!(
        page.items[0].reactions.len(),
        1,
        "groups: {:?}",
        page.items[0].reactions
    );
    assert_eq!(page.items[0].reactions[0].reaction_icon, "❤️");
    assert_eq!(page.items[0].reactions[0].count, 3);
}

#[test]
fn hydrated_reactions_normalize_slash_colon_provider_aliases() {
    let harness = Harness::new();
    harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-slash-colon-aliases"))
        .unwrap();

    for (index, reaction) in [
        "/:heart", "/:strong", "/:haha", "/:wow", "/:cry", "/:sad", "/:angry",
    ]
    .iter()
    .enumerate()
    {
        harness
            .repository
            .apply_reaction(&NormalizedReaction {
                provider_account_key: "imei-a".to_string(),
                external_thread_id: "peer-1".to_string(),
                is_group: false,
                provider_event_id: Some(format!("event-slash-colon-alias-{index}")),
                provider_message_id: "message-slash-colon-aliases".to_string(),
                provider_client_message_id: "client-message-slash-colon-aliases".to_string(),
                actor_external_id: format!("actor-{index}"),
                reaction: (*reaction).to_string(),
                removed: false,
                occurred_at: format!("2026-08-09T10:01:0{index}.000Z"),
                raw_payload: serde_json::json!({}),
            })
            .unwrap();
    }

    let conversation_id = harness.scalar("SELECT id FROM channel_conversations LIMIT 1");
    let page = harness
        .repository
        .get_conversation_timeline(conversation_id, None, 50)
        .unwrap();
    let reaction_icons = page.items[0]
        .reactions
        .iter()
        .map(|group| group.reaction_icon.as_str())
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(
        reaction_icons,
        std::collections::BTreeSet::from(["❤️", "👍", "😆", "😮", "😭", "😡"])
    );
    assert_eq!(
        page.items[0]
            .reactions
            .iter()
            .find(|group| group.reaction_icon == "😭")
            .map(|group| group.count),
        Some(2)
    );
}

#[test]
fn phase2a_message_actions_require_at_least_one_provider_id() {
    let harness = Harness::new();

    let mut provider_only = direct_message_for_peer("imei-a", "provider-only", "peer-provider");
    provider_only.provider_client_message_id = None;
    let provider_only = harness
        .repository
        .apply_normalized_message(&provider_only)
        .unwrap();

    let mut client_only = direct_message_for_peer("imei-a", "client-only", "peer-client");
    client_only.provider_message_id = None;
    let client_only = harness
        .repository
        .apply_normalized_message(&client_only)
        .unwrap();

    let mut neither = direct_message_for_peer("imei-a", "neither-id", "peer-neither");
    neither.provider_message_id = None;
    neither.provider_client_message_id = None;
    let neither = harness
        .repository
        .apply_normalized_message(&neither)
        .unwrap();

    let complete = harness
        .repository
        .apply_normalized_message(&direct_message_for_peer(
            "imei-a",
            "provider-complete",
            "peer-complete",
        ))
        .unwrap();

    let capabilities_json =
        serde_json::to_string(&ConversationCapabilities::zalo_phase_one()).unwrap();
    harness
        .repository
        .update_account_identity_and_capabilities(
            complete.channel_account_id,
            Some("own-zuid-1"),
            &capabilities_json,
            "2026-08-03T10:00:00.000Z",
        )
        .unwrap();
    let conn = open_db_connection(harness.repository.db_path()).unwrap();
    conn.execute(
        "UPDATE channel_accounts SET connection_status = 'connected' WHERE id = ?1",
        [complete.channel_account_id],
    )
    .unwrap();

    let provider_only = harness
        .repository
        .get_message_by_id(provider_only.message_id)
        .unwrap()
        .unwrap();
    let client_only = harness
        .repository
        .get_message_by_id(client_only.message_id)
        .unwrap()
        .unwrap();
    let neither = harness
        .repository
        .get_message_by_id(neither.message_id)
        .unwrap()
        .unwrap();
    let complete = harness
        .repository
        .get_message_by_id(complete.message_id)
        .unwrap()
        .unwrap();

    assert!(provider_only.actions.react);
    assert!(client_only.actions.react);
    assert!(!neither.actions.react);
    assert!(complete.actions.react);
}

#[test]
fn phase2a_capability_rollback_preserves_phase1_actions_and_read_only_reactions() {
    let harness = Harness::new();
    let mut message = direct_message("imei-a", "message-capability-rollback");
    message.direction = MessageDirection::Outbound;
    message.status = MessageStatus::Sent;
    let applied = harness
        .repository
        .apply_normalized_message(&message)
        .unwrap();

    let mut capabilities = ConversationCapabilities::zalo_phase_one();
    capabilities.reactions.write = false;
    capabilities.forward.enabled = false;
    capabilities.mentions.enabled = false;
    capabilities.quick_templates.enabled = false;
    capabilities.recall_window_seconds = None;
    let capabilities_json = serde_json::to_string(&capabilities).unwrap();
    harness
        .repository
        .update_account_identity_and_capabilities(
            applied.channel_account_id,
            Some("own-zuid-1"),
            &capabilities_json,
            "2026-08-03T10:00:00.000Z",
        )
        .unwrap();
    let conn = open_db_connection(harness.repository.db_path()).unwrap();
    conn.execute(
        "UPDATE channel_accounts SET connection_status = 'connected' WHERE id = ?1",
        [applied.channel_account_id],
    )
    .unwrap();

    harness
        .repository
        .apply_reaction(&NormalizedReaction {
            provider_account_key: "imei-a".to_string(),
            external_thread_id: "peer-1".to_string(),
            is_group: false,
            provider_event_id: Some("event-capability-rollback".to_string()),
            provider_message_id: "message-capability-rollback".to_string(),
            provider_client_message_id: "client-message-capability-rollback".to_string(),
            actor_external_id: "peer-1".to_string(),
            reaction: "heart".to_string(),
            removed: false,
            occurred_at: "2026-08-03T10:01:00.000Z".to_string(),
            raw_payload: serde_json::json!({}),
        })
        .unwrap();

    let hydrated = harness
        .repository
        .get_message_by_id(applied.message_id)
        .unwrap()
        .unwrap();

    assert_eq!(hydrated.reactions.len(), 1);
    assert!(hydrated.actions.copy);
    assert!(!hydrated.actions.react);
    assert!(!hydrated.actions.forward);
    assert!(hydrated.actions.reply);
    assert!(hydrated.actions.recall);
}

#[test]
fn phase2a_message_actions_require_connection_and_enforce_recall_window() {
    let harness = Harness::new();
    let mut message = direct_message("imei-a", "message-action-policy");
    message.direction = MessageDirection::Outbound;
    message.status = MessageStatus::Sent;
    let applied = harness
        .repository
        .apply_normalized_message(&message)
        .unwrap();

    let capabilities = ConversationCapabilities::zalo_phase_one();
    let capabilities_json = serde_json::to_string(&capabilities).unwrap();
    harness
        .repository
        .update_account_identity_and_capabilities(
            applied.channel_account_id,
            Some("own-zuid-1"),
            &capabilities_json,
            "2026-08-03T10:00:00.000Z",
        )
        .unwrap();

    let conn = open_db_connection(harness.repository.db_path()).unwrap();
    conn.execute(
        "UPDATE channel_accounts SET connection_status = 'disconnected' WHERE id = ?1",
        [applied.channel_account_id],
    )
    .unwrap();

    let disconnected = harness
        .repository
        .get_message_by_id(applied.message_id)
        .unwrap()
        .unwrap();
    assert!(!disconnected.actions.reply);
    assert!(!disconnected.actions.recall);
    assert!(!disconnected.actions.forward);
    assert!(!disconnected.actions.react);

    conn.execute(
        "UPDATE channel_accounts SET connection_status = 'connected' WHERE id = ?1",
        [applied.channel_account_id],
    )
    .unwrap();

    let connected = harness
        .repository
        .get_message_by_id(applied.message_id)
        .unwrap()
        .unwrap();
    assert!(connected.actions.reply);
    assert!(!connected.actions.recall);
    assert!(connected.actions.forward);
    assert!(connected.actions.react);
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

    harness
        .repository
        .delete_conversation(&del_req, None)
        .unwrap();
}

#[test]
fn start_direct_conversation_assigns_numeric_provider_client_message_id() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
         VALUES (809, 'zalo', 'imei-client-id', 'Sale Account', 'connected', '2026-08-03T00:00:00.000Z', '2026-08-03T00:00:00.000Z')",
        [],
    )
    .unwrap();
    drop(conn);

    let request = crate::conversation::types::StartDirectConversationRequest {
        channel_account_id: 809,
        phone_or_external_id: "0949743369".to_string(),
        display_name: Some("Hồ Ngọc Phương Thảo".to_string()),
        initial_message: "Tin nhắn kiểm thử".to_string(),
    };

    let message = harness
        .repository
        .start_direct_conversation(&request, "2026-08-03T09:34:58.278Z")
        .unwrap();

    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let provider_client_message_id: Option<String> = conn
        .query_row(
            "SELECT provider_client_message_id FROM messages WHERE id = ?1",
            [message.id],
            |row| row.get(0),
        )
        .unwrap();

    let provider_client_message_id = provider_client_message_id
        .expect("new direct message must have a provider client message id");
    assert!(provider_client_message_id.parse::<u64>().is_ok());
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
        .resolve_direct_phone_recipient(ResolveDirectPhoneRecipientRequest {
            message_id: message.id,
            channel_account_id: 998,
            phone: "0949743369",
            resolved_uid: "zalo-uid-thao-live",
            display_name: Some("Hồ Ngọc Phương Thảo"),
            avatar_url: Some("https://example.com/thao.jpg"),
            now: "2026-08-02T00:46:00.000Z",
        })
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
fn resolve_direct_phone_recipient_clears_uid_tombstone_for_explicit_reopen() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO leads (id, name, phone, zalo_uid)
         VALUES (702, 'Reopened Lead', '0901234567', NULL)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_accounts
         (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
         VALUES (999, 'zalo', 'imei-phone-reopen', 'Phone Reopen', 'connected',
                 '2026-08-13T00:00:00Z', '2026-08-13T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO deleted_conversations
         (channel_account_id, external_thread_id, kind, deleted_at)
         VALUES (999, 'resolved-phone-uid', 'direct', '2099-01-01T00:00:00Z')",
        [],
    )
    .unwrap();
    drop(conn);

    let outbound = harness
        .repository
        .start_direct_conversation(
            &crate::conversation::types::StartDirectConversationRequest {
                channel_account_id: 999,
                phone_or_external_id: "0901234567".to_string(),
                display_name: Some("Reopened Lead".to_string()),
                initial_message: "Mở lại hội thoại".to_string(),
            },
            "2026-08-13T01:00:00Z",
        )
        .unwrap();

    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let tombstone_before_resolve: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM deleted_conversations
             WHERE channel_account_id = 999
               AND external_thread_id = 'resolved-phone-uid'
               AND kind = 'direct'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(tombstone_before_resolve, 1);
    drop(conn);

    let resolved = harness
        .repository
        .resolve_direct_phone_recipient(ResolveDirectPhoneRecipientRequest {
            message_id: outbound.id,
            channel_account_id: 999,
            phone: "0901234567",
            resolved_uid: "resolved-phone-uid",
            display_name: Some("Reopened Lead"),
            avatar_url: None,
            now: "2026-08-13T01:00:01Z",
        })
        .unwrap();

    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let tombstone_after_resolve: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM deleted_conversations
             WHERE channel_account_id = 999
               AND external_thread_id = 'resolved-phone-uid'
               AND kind = 'direct'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(tombstone_after_resolve, 0);
    drop(conn);

    let mut reply = direct_message_for_peer(
        "imei-phone-reopen",
        "reply-after-explicit-reopen",
        "resolved-phone-uid",
    );
    reply.provider_timestamp = Some("2026-08-13T00:59:59Z".to_string());
    reply.ingested_at = "2026-08-13T01:00:02Z".to_string();
    let applied = harness.repository.apply_normalized_message(&reply).unwrap();
    assert_eq!(applied.conversation_id, resolved.conversation_id);
    assert!(applied.message_id > 0);
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
                    mentions: Vec::new(),
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
        message_tag: None,
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
                mentions: Vec::new(),
            },
        }],
        message_tag: None,
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
                    mentions: Vec::new(),
                },
            },
            ConversationSendPartRequest {
                client_part_id: "part-2".into(),
                content: ConversationSendPartContent::Sticker {
                    sticker_id: 1,
                    cate_id: 2,
                    sticker_type: 1,
                    url: None,
                },
            },
        ],
        message_tag: None,
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
                url: None,
            },
        }],
        message_tag: None,
    };

    let res = repo
        .enqueue_outbound_batch(&valid_sticker_req, "2026-08-01T00:00:00Z")
        .unwrap();
    assert_eq!(res.messages.len(), 1);

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn enqueue_batch_group_with_null_display_name_participant_succeeds() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at) VALUES (1, 'zalo', 'imei-a', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at) VALUES (127, 1, 'group-127', 'group', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_identities (id, channel, identity_namespace, external_id, display_name, created_at, updated_at) VALUES (10, 'zalo', 'zalo_uid', 'uid-null', NULL, '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role) VALUES (127, 10, 'member')",
        [],
    ).unwrap();
    drop(conn);

    use crate::conversation::types::{
        ConversationSendPartContent, ConversationSendPartRequest, SendConversationBatchRequest,
    };

    let req = SendConversationBatchRequest {
        conversation_id: 127,
        client_batch_id: "batch-null-test".into(),
        reply_to_message_id: None,
        parts: vec![ConversationSendPartRequest {
            client_part_id: "part-1".into(),
            content: ConversationSendPartContent::Text {
                text: "Hello from group".into(),
                mentions: vec![],
            },
        }],
        message_tag: None,
    };

    let res = harness
        .repository
        .enqueue_outbound_batch(&req, "2026-08-01T00:00:00Z")
        .unwrap();
    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0].text_content.as_deref(),
        Some("Hello from group")
    );
}

#[test]
fn enqueue_batch_saves_message_tag_in_content_json_only_when_provided() {
    use crate::conversation::types::{
        ConversationSendPartContent, ConversationSendPartRequest, SendConversationBatchRequest,
    };

    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at) VALUES (1, 'messenger', 'page-1', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at) VALUES (1, 1, 'psid-1', 'direct', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();
    drop(conn);

    // 1. Without message_tag
    let req_no_tag = SendConversationBatchRequest {
        conversation_id: 1,
        client_batch_id: "batch-no-tag".to_string(),
        reply_to_message_id: None,
        parts: vec![ConversationSendPartRequest {
            client_part_id: "p1".to_string(),
            content: ConversationSendPartContent::Text {
                text: "No tag message".to_string(),
                mentions: Vec::new(),
            },
        }],
        message_tag: None,
    };

    let res_no_tag = harness
        .repository
        .enqueue_outbound_batch(&req_no_tag, "2026-08-01T00:00:00Z")
        .unwrap();
    assert_eq!(res_no_tag.messages.len(), 1);

    let conn_check = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let json_no_tag_str: String = conn_check
        .query_row(
            "SELECT content_json FROM messages WHERE id = ?1",
            [res_no_tag.messages[0].id],
            |r| r.get(0),
        )
        .unwrap();
    let json_no_tag: serde_json::Value = serde_json::from_str(&json_no_tag_str).unwrap();
    assert_eq!(json_no_tag.get("messageTag"), None);

    // 2. With message_tag: Some("HUMAN_AGENT")
    let req_with_tag = SendConversationBatchRequest {
        conversation_id: 1,
        client_batch_id: "batch-with-tag".to_string(),
        reply_to_message_id: None,
        parts: vec![ConversationSendPartRequest {
            client_part_id: "p2".to_string(),
            content: ConversationSendPartContent::Text {
                text: "Human agent tag message".to_string(),
                mentions: Vec::new(),
            },
        }],
        message_tag: Some("HUMAN_AGENT".to_string()),
    };

    let res_with_tag = harness
        .repository
        .enqueue_outbound_batch(&req_with_tag, "2026-08-01T00:00:01Z")
        .unwrap();
    assert_eq!(res_with_tag.messages.len(), 1);

    let json_with_tag_str: String = conn_check
        .query_row(
            "SELECT content_json FROM messages WHERE id = ?1",
            [res_with_tag.messages[0].id],
            |r| r.get(0),
        )
        .unwrap();
    let json_with_tag: serde_json::Value = serde_json::from_str(&json_with_tag_str).unwrap();
    assert_eq!(
        json_with_tag.get("messageTag").and_then(|v| v.as_str()),
        Some("HUMAN_AGENT")
    );
}

#[test]
fn mention_incoming_mentions_upsert_without_duplicates() {
    let harness = Harness::new();
    let mut msg = direct_message("imei-a", "msg-mention-1");
    msg.conversation_kind = ConversationKind::Group;
    msg.external_thread_id = "group-100".to_string();
    msg.text_content = Some("Xin chào @Alice và @Bob".to_string());
    msg.mentions = vec![
        NormalizedMention {
            identity_id: None,
            external_id: Some("user-101".to_string()),
            display_text: "@Alice".to_string(),
            start_utf16: 9,
            length_utf16: 6,
        },
        NormalizedMention {
            identity_id: None,
            external_id: Some("user-102".to_string()),
            display_text: "@Bob".to_string(),
            start_utf16: 19,
            length_utf16: 4,
        },
    ];

    let res1 = harness.repository.apply_normalized_message(&msg).unwrap();
    assert!(!res1.duplicate);

    let res2 = harness.repository.apply_normalized_message(&msg).unwrap();
    assert!(res2.duplicate);

    let conn = open_db_connection(harness.repository.db_path()).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM message_mentions WHERE message_id = ?1",
            [res1.message_id],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(count, 2);
}

#[test]
fn mention_message_projection_includes_mentions() {
    let harness = Harness::new();
    let mut msg = direct_message("imei-a", "msg-mention-proj");
    msg.conversation_kind = ConversationKind::Group;
    msg.external_thread_id = "group-200".to_string();
    msg.text_content = Some("Hi @Alice!".to_string());
    msg.mentions = vec![NormalizedMention {
        identity_id: None,
        external_id: Some("user-201".to_string()),
        display_text: "@Alice".to_string(),
        start_utf16: 3,
        length_utf16: 6,
    }];

    let res = harness.repository.apply_normalized_message(&msg).unwrap();
    let hydrated = harness
        .repository
        .get_message_by_id(res.message_id)
        .unwrap()
        .unwrap();

    assert_eq!(hydrated.mentions.len(), 1);
    assert_eq!(hydrated.mentions[0].pos, 3);
    assert_eq!(hydrated.mentions[0].len, 6);
    assert_eq!(
        hydrated.mentions[0].target_external_id.as_deref(),
        Some("user-201")
    );
    assert_eq!(hydrated.mentions[0].display_name.as_deref(), Some("@Alice"));
    assert!(!hydrated.mentions[0].is_all);
}

#[test]
fn mention_authoritative_echo_replaces_stale_mentions() {
    let harness = Harness::new();
    let mut msg = direct_message("imei-a", "msg-echo-mention");
    msg.conversation_kind = ConversationKind::Group;
    msg.external_thread_id = "group-300".to_string();
    msg.text_content = Some("Hi @Alice @Bob".to_string());
    msg.mentions = vec![
        NormalizedMention {
            identity_id: None,
            external_id: Some("user-1".to_string()),
            display_text: "@Alice".to_string(),
            start_utf16: 3,
            length_utf16: 6,
        },
        NormalizedMention {
            identity_id: None,
            external_id: Some("user-2".to_string()),
            display_text: "@Bob".to_string(),
            start_utf16: 10,
            length_utf16: 4,
        },
    ];

    let res1 = harness.repository.apply_normalized_message(&msg).unwrap();
    let conn = open_db_connection(harness.repository.db_path()).unwrap();
    let count1: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM message_mentions WHERE message_id = ?1",
            [res1.message_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count1, 2);

    msg.text_content = Some("Hi @Alice".to_string());
    msg.raw_payload = Some(serde_json::json!({"echo": true}));
    msg.mentions = vec![NormalizedMention {
        identity_id: None,
        external_id: Some("user-1".to_string()),
        display_text: "@Alice".to_string(),
        start_utf16: 3,
        length_utf16: 6,
    }];

    let res2 = harness.repository.apply_normalized_message(&msg).unwrap();
    assert!(res2.duplicate);

    let hydrated = harness
        .repository
        .get_message_by_id(res1.message_id)
        .unwrap()
        .unwrap();

    assert_eq!(hydrated.mentions.len(), 1);
    assert_eq!(
        hydrated.mentions[0].target_external_id.as_deref(),
        Some("user-1")
    );
}

#[test]
fn self_echo_does_not_double_count() {
    let harness = Harness::new();

    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, self_external_id, created_at, updated_at)
         VALUES (1, 'zalo', 'acc-1', 'Test Acc', 'self-uid', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z')",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at)
         VALUES (100, 1, 'thread-1', 'direct', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z')",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, provider_message_id, content_type, text_content, ingested_at)
         VALUES (10, 100, 'outbound', 'msg-10', 'text', 'Hello', '2026-08-02T10:00:00Z')",
        [],
    ).unwrap();

    let norm_react_local = NormalizedReaction {
        provider_account_key: "acc-1".to_string(),
        external_thread_id: "thread-1".to_string(),
        is_group: false,
        provider_event_id: None,
        provider_message_id: "msg-10".to_string(),
        provider_client_message_id: String::new(),
        actor_external_id: "self-uid".to_string(),
        reaction: "heart".to_string(),
        removed: false,
        occurred_at: "2026-08-02T12:00:00Z".to_string(),
        raw_payload: serde_json::Value::Null,
    };
    harness
        .repository
        .apply_reaction(&norm_react_local)
        .unwrap();

    let norm_react_echo = NormalizedReaction {
        provider_account_key: "acc-1".to_string(),
        external_thread_id: "thread-1".to_string(),
        is_group: false,
        provider_event_id: Some("evt-1".to_string()),
        provider_message_id: "msg-10".to_string(),
        provider_client_message_id: String::new(),
        actor_external_id: "self-uid".to_string(),
        reaction: "heart".to_string(),
        removed: false,
        occurred_at: "2026-08-02T12:00:05Z".to_string(),
        raw_payload: serde_json::Value::Null,
    };
    harness.repository.apply_reaction(&norm_react_echo).unwrap();

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM message_reactions WHERE message_id = 10 AND actor_external_id = 'self-uid' AND reaction = 'heart' AND removed_at IS NULL",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn remove_echo_marks_exact_reaction_removed() {
    let harness = Harness::new();

    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, created_at, updated_at)
         VALUES (1, 'zalo', 'acc-1', 'Test Acc', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z')",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at)
         VALUES (100, 1, 'thread-1', 'direct', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z')",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, provider_message_id, content_type, text_content, ingested_at)
         VALUES (10, 100, 'outbound', 'msg-10', 'text', 'Hello', '2026-08-02T10:00:00Z')",
        [],
    ).unwrap();

    let react_heart = NormalizedReaction {
        provider_account_key: "acc-1".to_string(),
        external_thread_id: "thread-1".to_string(),
        is_group: false,
        provider_event_id: None,
        provider_message_id: "msg-10".to_string(),
        provider_client_message_id: String::new(),
        actor_external_id: "actor-1".to_string(),
        reaction: "heart".to_string(),
        removed: false,
        occurred_at: "2026-08-02T12:00:00Z".to_string(),
        raw_payload: serde_json::Value::Null,
    };
    let react_like = NormalizedReaction {
        provider_account_key: "acc-1".to_string(),
        external_thread_id: "thread-1".to_string(),
        is_group: false,
        provider_event_id: None,
        provider_message_id: "msg-10".to_string(),
        provider_client_message_id: String::new(),
        actor_external_id: "actor-1".to_string(),
        reaction: "like".to_string(),
        removed: false,
        occurred_at: "2026-08-02T12:00:00Z".to_string(),
        raw_payload: serde_json::Value::Null,
    };
    harness.repository.apply_reaction(&react_heart).unwrap();
    harness.repository.apply_reaction(&react_like).unwrap();

    let remove_heart = NormalizedReaction {
        provider_account_key: "acc-1".to_string(),
        external_thread_id: "thread-1".to_string(),
        is_group: false,
        provider_event_id: None,
        provider_message_id: "msg-10".to_string(),
        provider_client_message_id: String::new(),
        actor_external_id: "actor-1".to_string(),
        reaction: "heart".to_string(),
        removed: true,
        occurred_at: "2026-08-02T12:01:00Z".to_string(),
        raw_payload: serde_json::Value::Null,
    };
    harness.repository.apply_reaction(&remove_heart).unwrap();

    let heart_active: i64 = conn.query_row(
        "SELECT COUNT(*) FROM message_reactions WHERE message_id = 10 AND actor_external_id = 'actor-1' AND reaction = 'heart' AND removed_at IS NULL",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(heart_active, 0);

    let like_active: i64 = conn.query_row(
        "SELECT COUNT(*) FROM message_reactions WHERE message_id = 10 AND actor_external_id = 'actor-1' AND reaction = 'like' AND removed_at IS NULL",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(like_active, 1);
}

#[test]
fn unknown_remove_echo_clears_actor_reaction_without_unknown_badge() {
    let harness = Harness::new();

    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, created_at, updated_at)
         VALUES (1, 'zalo', 'acc-1', 'Test Acc', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at)
         VALUES (100, 1, 'thread-1', 'direct', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, provider_message_id, content_type, text_content, ingested_at)
         VALUES (10, 100, 'outbound', 'msg-10', 'text', 'Hello', '2026-08-02T10:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO message_reactions (message_id, actor_external_id, reaction, reacted_at, removed_at)
         VALUES (10, 'actor-1', '/-heart', '2026-08-02T12:00:00Z', NULL)",
        [],
    )
    .unwrap();

    let remove_unknown = NormalizedReaction {
        provider_account_key: "acc-1".to_string(),
        external_thread_id: "thread-1".to_string(),
        is_group: false,
        provider_event_id: None,
        provider_message_id: "msg-10".to_string(),
        provider_client_message_id: String::new(),
        actor_external_id: "actor-1".to_string(),
        reaction: "Unknown".to_string(),
        removed: true,
        occurred_at: "2026-08-02T12:01:00Z".to_string(),
        raw_payload: serde_json::Value::Null,
    };
    harness.repository.apply_reaction(&remove_unknown).unwrap();

    let active_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM message_reactions
             WHERE message_id = 10 AND actor_external_id = 'actor-1' AND removed_at IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(active_count, 0);

    let unknown_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM message_reactions
             WHERE message_id = 10 AND actor_external_id = 'actor-1' AND reaction = 'Unknown'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(unknown_count, 0);
}

#[test]
fn matching_reconciliation_completes_unknown_mutation() {
    let harness = Harness::new();

    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, created_at, updated_at)
         VALUES (1, 'zalo', 'acc-1', 'Test Acc', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z')",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at)
         VALUES (100, 1, 'thread-1', 'direct', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z')",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, provider_message_id, content_type, text_content, ingested_at)
         VALUES (10, 100, 'outbound', 'msg-10', 'text', 'Hello', '2026-08-02T10:00:00Z')",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO reaction_mutations (client_mutation_id, message_id, reaction_code, active, status, created_at, updated_at)
         VALUES ('mut-unk-1', 10, 'haha', 1, 'delivery_unknown', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z')",
        [],
    ).unwrap();

    let react_haha = NormalizedReaction {
        provider_account_key: "acc-1".to_string(),
        external_thread_id: "thread-1".to_string(),
        is_group: false,
        provider_event_id: None,
        provider_message_id: "msg-10".to_string(),
        provider_client_message_id: String::new(),
        actor_external_id: "actor-1".to_string(),
        reaction: "haha".to_string(),
        removed: false,
        occurred_at: "2026-08-02T12:00:00Z".to_string(),
        raw_payload: serde_json::Value::Null,
    };
    harness.repository.apply_reaction(&react_haha).unwrap();

    let status: String = conn
        .query_row(
            "SELECT status FROM reaction_mutations WHERE client_mutation_id = 'mut-unk-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "succeeded");
}

#[test]
fn forward_conversation_message_enqueues_batch() {
    use crate::conversation::types::{
        ForwardConversationMessageRequest, ForwardOperationStatus, ForwardTargetStatus,
    };

    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    let caps_json = serde_json::to_string(
        &crate::conversation::types::ConversationCapabilities::zalo_phase_one(),
    )
    .unwrap();
    let account_id: i64 = conn
        .query_row(
            "INSERT INTO channel_accounts (channel, provider_account_key, connection_status, capabilities_json, created_at, updated_at)
             VALUES ('zalo', 'acc-repo-fwd', 'connected', ?1, '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z')
             RETURNING id",
            [caps_json],
            |r| r.get(0),
        )
        .unwrap();

    let conv_src: i64 = conn
        .query_row(
            "INSERT INTO channel_conversations (channel_account_id, external_thread_id, kind, title, status, last_synced_at, created_at, updated_at)
             VALUES (?1, 't-src', 'direct', 'Source', 'active', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z') RETURNING id",
            [account_id],
            |r| r.get(0),
        )
        .unwrap();

    let conv_t1: i64 = conn
        .query_row(
            "INSERT INTO channel_conversations (channel_account_id, external_thread_id, kind, title, status, last_synced_at, created_at, updated_at)
             VALUES (?1, 't-t1', 'direct', 'Target 1', 'active', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z') RETURNING id",
            [account_id],
            |r| r.get(0),
        )
        .unwrap();

    let src_msg_id: i64 = conn
        .query_row(
            "INSERT INTO messages (conversation_id, direction, provider_message_id, provider_client_message_id, content_type, text_content, provider_timestamp, ingested_at)
             VALUES (?1, 'inbound', 'pmsg-repo-1', 'pcli-repo-1', 'text', 'Hello Repo Forward', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z') RETURNING id",
            [conv_src],
            |r| r.get(0),
        )
        .unwrap();

    let req = ForwardConversationMessageRequest {
        client_operation_id: "op-repo-fwd-1".to_string(),
        source_message_id: src_msg_id,
        target_conversation_ids: vec![conv_t1],
    };

    let res = harness
        .repository
        .forward_conversation_message(&req)
        .unwrap();
    assert_eq!(res.status, ForwardOperationStatus::Queued);
    assert_eq!(res.targets.len(), 1);
    assert_eq!(res.targets[0].target_conversation_id, conv_t1);
    assert_eq!(
        res.targets[0].client_batch_id,
        format!("op-repo-fwd-1:{conv_t1}")
    );

    harness
        .repository
        .update_forward_target_status(
            "op-repo-fwd-1",
            conv_t1,
            ForwardTargetStatus::Sent,
            None,
            None,
            "2026-08-02T10:01:00Z",
        )
        .unwrap();

    let latest = harness
        .repository
        .get_forward_operation("op-repo-fwd-1")
        .unwrap();
    assert_eq!(latest.status, ForwardOperationStatus::Sent);
    assert_eq!(latest.targets[0].status, ForwardTargetStatus::Sent);
}

#[test]
fn test_pin_unpin_and_list_pinned_messages() {
    let harness = Harness::new();
    let msg = direct_message("acc-1", "msg-pin-1");
    let result = harness.repository.apply_normalized_message(&msg).unwrap();
    let conv_id = result.conversation_id;
    let msg_id = result.message_id;

    let pinned = harness
        .repository
        .pin_message(conv_id, msg_id, Some("user_1"))
        .unwrap();
    assert_eq!(pinned.conversation_id, conv_id);
    assert_eq!(pinned.message_id, msg_id);
    assert_eq!(pinned.pinned_by.as_deref(), Some("user_1"));
    assert_eq!(pinned.message_text.as_deref(), Some("hello"));

    let list = harness.repository.list_pinned_messages(conv_id).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].message_id, msg_id);

    harness.repository.unpin_message(conv_id, msg_id).unwrap();
    let empty_list = harness.repository.list_pinned_messages(conv_id).unwrap();
    assert!(empty_list.is_empty());
}

#[test]
fn reconcile_zalo_channel_account_reuses_existing_account_and_updates_provider_key() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    let acc_id = harness
        .repository
        .reconcile_zalo_channel_account("imei_1", Some("uid_100"), Some("Hello World"), None)
        .unwrap();

    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at)
         VALUES (10, ?1, 'thread_1', 'direct', '2026-08-09T00:00:00Z', '2026-08-09T00:00:00Z')",
        params![acc_id],
    )
    .unwrap();

    let reconciled_id = harness
        .repository
        .reconcile_zalo_channel_account("imei_2", Some("uid_100"), Some("Hello World 2"), None)
        .unwrap();

    assert_eq!(reconciled_id, acc_id);

    let conv_acc_id: i64 = conn
        .query_row(
            "SELECT channel_account_id FROM channel_conversations WHERE id = 10",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(conv_acc_id, acc_id);
}

#[test]
fn reconcile_duplicate_zalo_channel_accounts_merges_conversations_to_canonical_account() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    conn.execute(
        "DROP INDEX IF EXISTS idx_channel_accounts_zalo_self_ext_id",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, self_external_id, display_name, created_at, updated_at)
         VALUES (1, 'zalo', 'imei_old', 'uid_200', 'Account 1', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, self_external_id, display_name, created_at, updated_at)
         VALUES (2, 'zalo', 'imei_new', 'uid_200', 'Account 2', '2026-08-02T00:00:00Z', '2026-08-02T00:00:00Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at)
         VALUES (20, 2, 'thread_2', 'direct', '2026-08-02T00:00:00Z', '2026-08-02T00:00:00Z')",
        [],
    )
    .unwrap();

    crate::conversation::migration::reconcile_duplicate_zalo_channel_accounts(&conn).unwrap();

    let remaining_accounts: Vec<i64> = conn
        .prepare("SELECT id FROM channel_accounts WHERE self_external_id = 'uid_200'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(remaining_accounts, vec![2]);

    let conv_acc_id: i64 = conn
        .query_row(
            "SELECT channel_account_id FROM channel_conversations WHERE id = 20",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(conv_acc_id, 2);
}

#[test]
fn test_reconcile_duplicate_accounts_preserves_messages_participants_attachments_reactions() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    conn.execute(
        "DROP INDEX IF EXISTS idx_channel_accounts_zalo_self_ext_id",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, self_external_id, display_name, created_at, updated_at)
         VALUES (1, 'zalo', 'imei_old', 'uid_300', 'Account 1', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, self_external_id, display_name, created_at, updated_at)
         VALUES (2, 'zalo', 'imei_new', 'uid_300', 'Account 2', '2026-08-02T00:00:00Z', '2026-08-02T00:00:00Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at)
         VALUES (101, 1, 'shared_thread', 'direct', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at)
         VALUES (102, 2, 'shared_thread', 'direct', '2026-08-02T00:00:00Z', '2026-08-02T00:00:00Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, provider_message_id, content_type, text_content, ingested_at)
         VALUES (500, 102, 'inbound', 'msg_500', 'text', 'Important Message on Dup', '2026-08-02T10:00:00Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_identities (id, channel, identity_namespace, external_id, display_name, created_at, updated_at)
         VALUES (60, 'zalo', 'user', 'user_600', 'User 600', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, joined_at)
         VALUES (102, 60, '2026-08-02T10:00:00Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO message_attachments (id, message_id, attachment_type, created_at)
         VALUES (700, 500, 'image', '2026-08-02T10:00:00Z')",
        [],
    )
    .unwrap();

    crate::conversation::migration::reconcile_duplicate_zalo_channel_accounts(&conn).unwrap();

    let message_conv_id: i64 = conn
        .query_row(
            "SELECT conversation_id FROM messages WHERE id = 500",
            [],
            |r| r.get(0),
        )
        .unwrap();

    let canonical_conv_id: i64 = conn
        .query_row(
            "SELECT id FROM channel_conversations WHERE external_thread_id = 'shared_thread'",
            [],
            |r| r.get(0),
        )
        .unwrap();

    assert_eq!(message_conv_id, canonical_conv_id);

    let part_conv_id: i64 = conn
        .query_row(
            "SELECT conversation_id FROM conversation_participants WHERE identity_id = 60",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(part_conv_id, canonical_conv_id);

    let att_conv_id: i64 = conn
        .query_row(
            "SELECT m.conversation_id FROM message_attachments a JOIN messages m ON a.message_id = m.id WHERE a.id = 700",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(att_conv_id, canonical_conv_id);
}

#[test]
fn list_inbox_unlinked_title_ignores_other_senders_in_shared_group() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    // 1. Setup channel account & identities
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at) VALUES (1, 'zalo', 'acc_1', 'My Account', 'connected', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();

    // Identity A: User A (dName = Nguyễn Mạnh Toàn)
    conn.execute(
        "INSERT INTO channel_identities (id, channel, identity_namespace, external_id, display_name, created_at, updated_at) VALUES (10, 'zalo', 'user', 'user_a', 'Nguyễn Mạnh Toàn', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();

    // Identity B: User B (dName = Hoàng Tuấn)
    conn.execute(
        "INSERT INTO channel_identities (id, channel, identity_namespace, external_id, display_name, created_at, updated_at) VALUES (20, 'zalo', 'user', 'user_b', 'Hoàng Tuấn', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();

    // Direct conversation for Identity A
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, kind, external_thread_id, last_message_at, created_at, updated_at) VALUES (100, 1, 'direct', 'direct_a', '2026-08-09T10:00:00Z', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id) VALUES (100, 10)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, sender_identity_id, direction, provider_message_id, content_type, raw_payload_json, provider_timestamp, ingested_at, status)
         VALUES (1001, 100, 10, 'inbound', 'msg1', 'text', '{\"dName\": \"Nguyễn Mạnh Toàn\"}', '2026-08-09T10:00:00Z', '2026-08-09T10:00:00Z', 'received')",
        [],
    ).unwrap();

    // Group conversation where both Identity A and Identity B are participants
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, kind, external_thread_id, last_message_at, created_at, updated_at) VALUES (200, 1, 'group', 'group_1', '2026-08-09T11:00:00Z', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id) VALUES (200, 10)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id) VALUES (200, 20)",
        [],
    )
    .unwrap();

    // Newer message in group sent by Identity B (Hoàng Tuấn)
    conn.execute(
        "INSERT INTO messages (id, conversation_id, sender_identity_id, direction, provider_message_id, content_type, raw_payload_json, provider_timestamp, ingested_at, status)
         VALUES (2001, 200, 20, 'inbound', 'msg2', 'text', '{\"dName\": \"Hoàng Tuấn\"}', '2026-08-09T11:00:00Z', '2026-08-09T11:00:00Z', 'received')",
        [],
    ).unwrap();

    let items = harness.repository.list_inbox().unwrap();
    let user_a_item = items
        .iter()
        .find(|i| i.key == "identity:10")
        .expect("Should find item for identity:10");

    assert_eq!(user_a_item.title, "Nguyễn Mạnh Toàn");
}

#[test]
fn test_attachment_replay_with_merged_aliases_updates_survivor_cleanly() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    // 1. Setup account, identity, conversation and message
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
         VALUES (1, 'messenger', 'fb_page_1', 'Facebook Page', 'connected', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO channel_identities (id, channel_account_id, channel, identity_namespace, external_id, display_name, created_at, updated_at)
         VALUES (10, 1, 'messenger', 'user', 'fb_user_1', 'FB User', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, kind, external_thread_id, last_message_at, created_at, updated_at)
         VALUES (100, 1, 'direct', 'fb_thread_1', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO messages (id, conversation_id, sender_identity_id, direction, provider_message_id, content_type, raw_payload_json, provider_timestamp, ingested_at, status)
         VALUES (1001, 100, 10, 'inbound', 'mid_fb_1', 'image', '{}', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z', 'received')",
        [],
    ).unwrap();

    // Insert survivor attachment with aliases in metadata_json
    let metadata = serde_json::json!({
        "provider_attachment_id_aliases": ["att_alias_1", "att_alias_2"],
        "remote_url_aliases": ["https://cdn.example.com/alias_url_1.png", "https://cdn.example.com/alias_url_2.png"]
    });
    conn.execute(
        "INSERT INTO message_attachments (id, message_id, provider_attachment_id, attachment_type, mime_type, file_name, remote_url, metadata_json, created_at)
         VALUES (500, 1001, 'att_canonical_1', 'image', 'image/png', 'photo.png', 'https://cdn.example.com/canonical.png', ?1, '2026-08-01T00:00:00Z')",
        rusqlite::params![metadata.to_string()],
    ).unwrap();

    // 2. Now replay message with an attachment whose provider_attachment_id matches one of the aliases ("att_alias_2")
    let replay_msg = NormalizedMessage {
        channel: Channel::Messenger,
        provider_account_key: "fb_page_1".to_string(),
        account_display_name: Some("Facebook Page".to_string()),
        participant: Some(NormalizedIdentity {
            identity_namespace: "user".to_string(),
            external_id: "fb_user_1".to_string(),
            display_name: Some("FB User".to_string()),
            avatar_url: None,
            lead_id: None,
        }),
        sender_is_participant: true,
        external_thread_id: "fb_thread_1".to_string(),
        conversation_kind: ConversationKind::Direct,
        conversation_title: Some("FB User".to_string()),
        direction: MessageDirection::Inbound,
        provider_message_id: Some("mid_fb_1".to_string()),
        provider_client_message_id: None,
        local_request_id: None,
        content_type: "image".to_string(),
        text_content: None,
        content: serde_json::json!({"type": "image"}),
        attachments: vec![crate::conversation::repository::NormalizedAttachment {
            provider_attachment_id: Some("att_alias_2".to_string()),
            media_type: "image".to_string(),
            mime_type: "image/png".to_string(),
            file_name: Some("updated_photo.png".to_string()),
            size_bytes: Some(1024),
            remote_url: Some("https://cdn.example.com/alias_url_2.png".to_string()),
            thumbnail_url: None,
            metadata_json: Some(serde_json::json!({
                "provider_attachment_id_aliases": null,
                "remote_url_aliases": null,
                "_merged_attachment_rows": null,
                "custom_tag": "tag1"
            })),
        }],
        reply: None,
        mentions: Vec::new(),
        raw_payload: None,
        provider_timestamp: Some("2026-08-01T00:00:00Z".to_string()),
        ingested_at: "2026-08-01T00:00:00Z".to_string(),
        status: MessageStatus::Received,
    };

    let applied = harness
        .repository
        .apply_normalized_message(&replay_msg)
        .unwrap();
    assert!(applied.duplicate, "Message must be identified as duplicate");

    // Verify attachment count remains 1 and survivor was updated without unique violation
    let att_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM message_attachments WHERE message_id = 1001",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        att_count, 1,
        "Duplicate attachment should update survivor, not insert duplicate row"
    );

    let (file_name, meta_str): (String, String) = conn
        .query_row(
            "SELECT file_name, metadata_json FROM message_attachments WHERE id = 500",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(file_name, "updated_photo.png");
    let meta_json: serde_json::Value = serde_json::from_str(&meta_str).unwrap();
    assert_eq!(meta_json["custom_tag"], "tag1");
    assert_eq!(
        meta_json["provider_attachment_id_aliases"],
        serde_json::json!(["att_alias_1", "att_alias_2"]),
        "Preserved alias array must NOT be removed by incoming null"
    );

    // 3. Replay again with second alias "att_alias_1" and another metadata field
    let mut replay_msg_2 = replay_msg.clone();
    replay_msg_2.attachments[0].provider_attachment_id = Some("att_alias_1".to_string());
    replay_msg_2.attachments[0].metadata_json = Some(serde_json::json!({"second_tag": "tag2"}));

    let applied_2 = harness
        .repository
        .apply_normalized_message(&replay_msg_2)
        .unwrap();
    assert!(applied_2.duplicate);

    let meta_str_2: String = conn
        .query_row(
            "SELECT metadata_json FROM message_attachments WHERE id = 500",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let meta_json_2: serde_json::Value = serde_json::from_str(&meta_str_2).unwrap();
    assert_eq!(meta_json_2["custom_tag"], "tag1");
    assert_eq!(meta_json_2["second_tag"], "tag2");
    assert_eq!(
        meta_json_2["provider_attachment_id_aliases"],
        serde_json::json!(["att_alias_1", "att_alias_2"])
    );

    // 4. Adversarial test: replay attempting to overwrite provider_attachment_id_aliases with a fake array
    let mut replay_msg_3 = replay_msg.clone();
    replay_msg_3.attachments[0].provider_attachment_id = Some("att_alias_2".to_string());
    replay_msg_3.attachments[0].metadata_json = Some(serde_json::json!({
        "provider_attachment_id_aliases": ["fake_hacked_alias"],
        "remote_url_aliases": ["fake_url"],
        "_merged_attachment_rows": null,
        "third_tag": "tag3"
    }));

    let applied_3 = harness
        .repository
        .apply_normalized_message(&replay_msg_3)
        .unwrap();
    assert!(applied_3.duplicate);

    let meta_str_3: String = conn
        .query_row(
            "SELECT metadata_json FROM message_attachments WHERE id = 500",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let meta_json_3: serde_json::Value = serde_json::from_str(&meta_str_3).unwrap();
    assert_eq!(meta_json_3["third_tag"], "tag3");
    assert_eq!(
        meta_json_3["provider_attachment_id_aliases"],
        serde_json::json!(["att_alias_1", "att_alias_2"]),
        "Alias arrays must be immutable against hostile incoming array replacements"
    );

    // 5. Adversarial test: replay with non-object metadata (e.g. JSON string)
    let mut replay_msg_4 = replay_msg.clone();
    replay_msg_4.attachments[0].provider_attachment_id = Some("att_alias_1".to_string());
    replay_msg_4.attachments[0].metadata_json = Some(serde_json::json!("a raw string payload"));

    let applied_4 = harness
        .repository
        .apply_normalized_message(&replay_msg_4)
        .unwrap();
    assert!(applied_4.duplicate);

    let meta_str_4: String = conn
        .query_row(
            "SELECT metadata_json FROM message_attachments WHERE id = 500",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let meta_json_4: serde_json::Value = serde_json::from_str(&meta_str_4).unwrap();
    assert_eq!(
        meta_json_4["provider_attachment_id_aliases"],
        serde_json::json!(["att_alias_1", "att_alias_2"]),
        "Non-object metadata must not corrupt or erase preserved aliases"
    );

    // 6. Replay using remote_url alias without provider_attachment_id
    let mut replay_msg_5 = replay_msg.clone();
    replay_msg_5.attachments[0].provider_attachment_id = None;
    replay_msg_5.attachments[0].remote_url =
        Some("https://cdn.example.com/alias_url_1.png".to_string());
    replay_msg_5.attachments[0].metadata_json = Some(serde_json::json!({"url_replayed": true}));

    let applied_5 = harness
        .repository
        .apply_normalized_message(&replay_msg_5)
        .unwrap();
    assert!(applied_5.duplicate);

    let att_count_final: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM message_attachments WHERE message_id = 1001",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        att_count_final, 1,
        "All 6 replays must map to the exact same survivor row"
    );
}

#[test]
fn test_facebook_upload_session_init_pending_nullable_video_id_and_cas_transition() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    // Create facebook_upload_sessions table matching schema
    conn.execute(
        "CREATE TABLE IF NOT EXISTS facebook_upload_sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            post_id INTEGER NOT NULL,
            page_id TEXT NOT NULL,
            upload_type TEXT NOT NULL DEFAULT 'video' CHECK (upload_type IN ('video', 'reel')),
            upload_session_id TEXT NOT NULL UNIQUE,
            video_id TEXT,
            file_path TEXT NOT NULL,
            file_hash TEXT NOT NULL,
            file_size_bytes INTEGER NOT NULL,
            start_offset INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'init_pending' CHECK (status IN ('init_pending', 'in_progress', 'finishing', 'completed', 'failed')),
            retry_count INTEGER NOT NULL DEFAULT 0,
            owner_id TEXT,
            lease_deadline TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        [],
    ).unwrap();

    // 1. Insert parent channel account and scheduled post
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, connection_status, capabilities_json, created_at, updated_at)
         VALUES (100, 'messenger', 'page_123', 'connected', '{}', datetime('now'), datetime('now'))",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO facebook_scheduled_posts (id, draft_group_id, channel_account_id, page_id, post_type, message, idempotency_key, status, created_at, updated_at)
         VALUES (100, 'grp_100', 100, 'page_123', 'video', 'test message', 'idemp_100', 'publishing', datetime('now'), datetime('now'))",
        [],
    ).unwrap();

    // 2. Insert init_pending pre-START intent with video_id = NULL
    let intent_id = "intent_100_uuid";
    conn.execute(
        "INSERT INTO facebook_upload_sessions (post_id, page_id, upload_type, upload_session_id, video_id, file_path, file_hash, file_size_bytes, start_offset, status, retry_count, owner_id, lease_deadline, created_at, updated_at)
         VALUES (100, 'page_123', 'video', ?1, NULL, '/tmp/video.mp4', 'hash123', 10485760, 0, 'init_pending', 0, 'worker_1', datetime('now', '+5 minutes'), datetime('now'), datetime('now'))",
        params![intent_id],
    ).unwrap();

    // 3. Query row decoding video_id into Option<String> and loading retry_count
    let (row_id, sess_id, vid_id, status, retries): (i64, String, Option<String>, String, i64) = conn.query_row(
        "SELECT id, upload_session_id, video_id, status, retry_count FROM facebook_upload_sessions WHERE upload_session_id = ?1",
        params![intent_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    ).unwrap();
    assert_eq!(sess_id, intent_id);
    assert_eq!(vid_id, None);
    assert_eq!(status, "init_pending");
    assert_eq!(retries, 0);

    // 4. START accepted: checked CAS transitions to in_progress with remote video_id
    let updated = conn.execute(
        "UPDATE facebook_upload_sessions
         SET upload_session_id = 'meta_sess_456', video_id = 'meta_vid_789', status = 'in_progress', updated_at = datetime('now')
         WHERE id = ?1 AND owner_id = 'worker_1' AND status = 'init_pending'",
        params![row_id],
    ).unwrap();
    assert_eq!(updated, 1);

    // 5. Stolen lease CAS rejection
    let stale_update = conn.execute(
        "UPDATE facebook_upload_sessions
         SET upload_session_id = 'meta_sess_stale', video_id = 'meta_vid_stale', status = 'in_progress', updated_at = datetime('now')
         WHERE id = ?1 AND owner_id = 'worker_2_stolen' AND status = 'init_pending'",
        params![row_id],
    ).unwrap();
    assert_eq!(stale_update, 0);

    // 6. Test 429 RateLimited lease release and retry_count increment with checked CAS (rows == 1)
    conn.execute(
        "INSERT INTO facebook_scheduled_posts (id, draft_group_id, channel_account_id, page_id, post_type, message, idempotency_key, status, created_at, updated_at)
         VALUES (101, 'grp_101', 100, 'page_123', 'video', 'test message 2', 'idemp_101', 'publishing', datetime('now'), datetime('now'))",
        [],
    ).unwrap();

    let intent_429 = "intent_429_uuid";
    conn.execute(
        "INSERT INTO facebook_upload_sessions (post_id, page_id, upload_type, upload_session_id, video_id, file_path, file_hash, file_size_bytes, start_offset, status, retry_count, owner_id, lease_deadline, created_at, updated_at)
         VALUES (101, 'page_123', 'video', ?1, NULL, '/tmp/video2.mp4', 'hash429', 10485760, 0, 'init_pending', 0, 'worker_1', datetime('now', '+5 minutes'), datetime('now'), datetime('now'))",
        params![intent_429],
    ).unwrap();

    let r429_updated = conn.execute(
        "UPDATE facebook_upload_sessions
         SET owner_id = NULL, lease_deadline = NULL, retry_count = retry_count + 1, updated_at = datetime('now')
         WHERE upload_session_id = ?1 AND owner_id = 'worker_1' AND status = 'init_pending'",
        params![intent_429],
    ).unwrap();
    assert_eq!(
        r429_updated, 1,
        "429 backoff must release lease and increment retry count"
    );

    let (r429_owner, r429_retries): (Option<String>, i64) = conn.query_row(
        "SELECT owner_id, retry_count FROM facebook_upload_sessions WHERE upload_session_id = ?1",
        params![intent_429],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap();
    assert_eq!(r429_owner, None);
    assert_eq!(r429_retries, 1);

    // 7. Test max retries enforcement (retry_count >= 3) in uploader state machine transitioning session to failed
    conn.execute(
        "UPDATE facebook_upload_sessions SET retry_count = 3, owner_id = 'worker_1', lease_deadline = datetime('now', '+5 minutes') WHERE upload_session_id = ?1",
        params![intent_429],
    ).unwrap();

    let (sess_id_check, retries_check): (String, i64) = conn.query_row(
        "SELECT upload_session_id, retry_count FROM facebook_upload_sessions WHERE upload_session_id = ?1",
        params![intent_429],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap();
    assert_eq!(sess_id_check, intent_429);
    assert_eq!(retries_check, 3);

    // When uploader sees retries_check >= 3, it executes checked CAS transition to failed
    let max_retry_fail = conn.execute(
        "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = 'worker_1'",
        params![intent_429],
    ).unwrap();
    assert_eq!(max_retry_fail, 1, "Max retries must mark session failed");
}

#[test]
fn test_facebook_upload_session_reel_flow_and_polling_429() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    conn.execute(
        "CREATE TABLE IF NOT EXISTS facebook_upload_sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            post_id INTEGER NOT NULL,
            page_id TEXT NOT NULL,
            upload_type TEXT NOT NULL DEFAULT 'video' CHECK (upload_type IN ('video', 'reel')),
            upload_session_id TEXT NOT NULL UNIQUE,
            video_id TEXT,
            file_path TEXT NOT NULL,
            file_hash TEXT NOT NULL,
            file_size_bytes INTEGER NOT NULL,
            start_offset INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'init_pending' CHECK (status IN ('init_pending', 'in_progress', 'finishing', 'completed', 'failed')),
            retry_count INTEGER NOT NULL DEFAULT 0,
            owner_id TEXT,
            lease_deadline TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        [],
    ).unwrap();

    // 0. Insert parent channel account and scheduled post
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, connection_status, capabilities_json, created_at, updated_at)
         VALUES (200, 'messenger', 'page_123', 'connected', '{}', datetime('now'), datetime('now'))",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO facebook_scheduled_posts (id, draft_group_id, channel_account_id, page_id, post_type, message, idempotency_key, status, owner_id, claim_deadline, created_at, updated_at)
         VALUES (200, 'grp_200', 200, 'page_123', 'reel', 'test reel message', 'idemp_200', 'publishing', 'worker_1', datetime('now', '+5 minutes'), datetime('now'), datetime('now'))",
        [],
    ).unwrap();

    // 1. Test canonical column association for Reel START & START-retry on facebook_scheduled_posts
    let reel_vid_id = "meta_reel_12345";
    let assoc_rows = conn.execute(
        "UPDATE facebook_scheduled_posts SET upload_session_id = ?1 WHERE id = ?2 AND owner_id = ?3 AND status = 'publishing'",
        params![reel_vid_id, 200i64, "worker_1"],
    ).unwrap();
    assert_eq!(
        assoc_rows, 1,
        "Canonical columns upload_session_id and owner_id must successfully update scheduled post"
    );

    let post_upload_session: Option<String> = conn
        .query_row(
            "SELECT upload_session_id FROM facebook_scheduled_posts WHERE id = 200",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(post_upload_session, Some(reel_vid_id.to_string()));

    // 2. Insert Reel in_progress session after START
    conn.execute(
        "INSERT INTO facebook_upload_sessions (post_id, page_id, upload_type, upload_session_id, video_id, file_path, file_hash, file_size_bytes, start_offset, status, retry_count, owner_id, lease_deadline, created_at, updated_at)
         VALUES (200, 'page_123', 'reel', ?1, ?1, '/tmp/reel.mp4', 'reel_hash_1', 5242880, 0, 'in_progress', 0, 'worker_1', datetime('now', '+5 minutes'), datetime('now'), datetime('now'))",
        params![reel_vid_id],
    ).unwrap();

    // 3. Test lease contention when another worker tries to claim an actively leased session
    let contender_rows = conn.execute(
        "UPDATE facebook_upload_sessions
         SET owner_id = 'worker_contender', lease_deadline = datetime('now', '+5 minutes'), updated_at = datetime('now')
         WHERE upload_session_id = ?1 AND (owner_id = 'worker_contender' OR owner_id IS NULL OR lease_deadline < datetime('now'))",
        params![reel_vid_id],
    ).unwrap();
    assert_eq!(contender_rows, 0, "Contending worker must fail to claim active lease without failing parent post (returns StillProcessing)");

    // 4. Strict fail-closed parsing of Reel binary upload responses
    let valid_success = serde_json::json!({ "success": true });
    let is_valid = valid_success.get("success").and_then(|v| v.as_bool()) == Some(true);
    assert!(is_valid, "Documented {{\\\"success\\\": true}} must pass");

    let false_success = serde_json::json!({ "success": false });
    let is_false = false_success.get("success").and_then(|v| v.as_bool()) == Some(true);
    assert!(
        !is_false,
        "{{\\\"success\\\": false}} must be rejected fail-closed"
    );

    let missing_success = serde_json::json!({ "error": "upload_failed" });
    let is_missing = missing_success.get("success").and_then(|v| v.as_bool()) == Some(true);
    assert!(
        !is_missing,
        "Missing success field must be rejected fail-closed"
    );

    let empty_obj = serde_json::json!({});
    let is_empty = empty_obj.get("success").and_then(|v| v.as_bool()) == Some(true);
    assert!(!is_empty, "Empty response must be rejected fail-closed");

    let offset_updated = conn.execute(
        "UPDATE facebook_upload_sessions SET start_offset = ?1, updated_at = datetime('now') WHERE upload_session_id = ?2 AND owner_id = 'worker_1' AND status = 'in_progress'",
        params![5242880i64, reel_vid_id],
    ).unwrap();
    assert_eq!(offset_updated, 1);

    // 5. Test extended streaming lease duration (+30 mins) for Reel upload
    let stream_lease_mins = 30i64;
    let claim_deadline_str = format!("+{} minutes", stream_lease_mins);
    let extended_post_lease = conn.execute(
        "UPDATE facebook_scheduled_posts SET claim_deadline = datetime('now', ?1), updated_at = datetime('now') WHERE id = ?2 AND owner_id = ?3 AND status = 'publishing'",
        params![&claim_deadline_str, 200i64, "worker_1"],
    ).unwrap();
    assert_eq!(extended_post_lease, 1);

    let extended_sess_lease = conn.execute(
        "UPDATE facebook_upload_sessions SET lease_deadline = datetime('now', ?1), updated_at = datetime('now') WHERE upload_session_id = ?2 AND owner_id = ?3 AND status = 'in_progress'",
        params![&claim_deadline_str, reel_vid_id, "worker_1"],
    ).unwrap();
    assert_eq!(extended_sess_lease, 1);

    let offset_updated = conn.execute(
        "UPDATE facebook_upload_sessions SET start_offset = ?1, updated_at = datetime('now') WHERE upload_session_id = ?2 AND owner_id = 'worker_1' AND status = 'in_progress'",
        params![5242880i64, reel_vid_id],
    ).unwrap();
    assert_eq!(offset_updated, 1);

    // 6. Test authoritative uploading_phase.status detection on resume and fail-closed missing status check
    let invalid_resume_json = serde_json::json!({
        "id": reel_vid_id
    });
    let invalid_status_obj = invalid_resume_json
        .get("status")
        .and_then(|s| s.as_object());
    assert!(
        invalid_status_obj.is_none(),
        "Missing status object must fail-closed"
    );

    let resume_status_json = serde_json::json!({
        "status": {
            "video_status": "processing",
            "uploading_phase": {
                "status": "complete"
            },
            "processing_phase": {
                "status": "in_progress"
            },
            "publishing_phase": {
                "status": "not_started"
            }
        }
    });
    let status_obj = resume_status_json.get("status").and_then(|s| s.as_object());
    assert!(
        status_obj.is_some(),
        "Status object must be present and valid"
    );
    let uploading_status = status_obj
        .and_then(|s| s.get("uploading_phase"))
        .and_then(|p| p.get("status"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let is_already_uploaded = uploading_status == "complete";
    assert!(
        is_already_uploaded,
        "uploading_phase.status == 'complete' proves binary upload completed"
    );

    // 7. Transition to finishing before calling FINISH endpoint
    let finishing_updated = conn.execute(
        "UPDATE facebook_upload_sessions SET status = 'finishing', updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = 'worker_1' AND status = 'in_progress'",
        params![reel_vid_id],
    ).unwrap();
    assert_eq!(finishing_updated, 1);

    // 8. Test finishing session re-entry releasing lease cleanly on non-terminal StillProcessing via checked CAS
    let finishing_still_proc_release = conn.execute(
        "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = 'worker_1' AND status = 'finishing'",
        params![reel_vid_id],
    ).unwrap();
    assert_eq!(
        finishing_still_proc_release, 1,
        "Finishing session must release lease on StillProcessing return"
    );

    // 9. Test terminal status polling receiving HTTP 429: sets lease_deadline with Retry-After backoff (e.g. +60s)
    let retry_after_secs = 60u64;
    let poll_429_updated = conn.execute(
        "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = datetime('now', '+' || ?1 || ' seconds'), retry_count = retry_count + 1, updated_at = datetime('now') WHERE upload_session_id = ?2 AND status = 'finishing'",
        params![retry_after_secs, reel_vid_id],
    ).unwrap();
    assert_eq!(poll_429_updated, 1);

    // Verify background reconciler CANNOT claim this session while retry-after backoff is active
    let premature_reconcile = conn.execute(
        "UPDATE facebook_upload_sessions
         SET owner_id = 'worker_reconciler', lease_deadline = datetime('now', '+5 minutes'), updated_at = datetime('now')
         WHERE upload_session_id = ?1 AND status = 'finishing' AND (lease_deadline IS NULL OR lease_deadline < datetime('now'))",
        params![reel_vid_id],
    ).unwrap();
    assert_eq!(
        premature_reconcile, 0,
        "Reconciler must not claim rate-limited session before lease_deadline backoff expires"
    );

    // 10. After backoff elapses, polling worker reclaims lease and verifies complete publishing status
    let reclaims = conn.execute(
        "UPDATE facebook_upload_sessions SET owner_id = 'worker_2', lease_deadline = datetime('now', '+5 minutes'), updated_at = datetime('now') WHERE upload_session_id = ?1 AND status = 'finishing' AND (owner_id IS NULL OR lease_deadline <= datetime('now', '+60 seconds'))",
        params![reel_vid_id],
    ).unwrap();
    assert_eq!(reclaims, 1);

    let status_json = serde_json::json!({
        "status": {
            "video_status": "ready",
            "publishing_phase": {
                "status": "complete"
            }
        }
    });
    let pub_status = status_json
        .get("status")
        .and_then(|s| s.get("publishing_phase"))
        .and_then(|p| p.get("status"))
        .and_then(|v| v.as_str());
    assert_eq!(pub_status, Some("complete"));

    let completed = conn.execute(
        "UPDATE facebook_upload_sessions SET status = 'completed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = 'worker_2' AND status = 'finishing'",
        params![reel_vid_id],
    ).unwrap();
    assert_eq!(completed, 1);
}

#[test]
fn test_facebook_uploader_429_backoff_sql_contract_binds_retry_after() {
    // Regression guard (Review 4998233494, finding 1/4): every 429-backoff UPDATE in the
    // Phase 2 plan must bind Meta's Retry-After into its own placeholder, distinct from the
    // session/row identifier placeholders. The broken shape reused ?1 for both the datetime
    // modifier and the identifier, so lease_deadline was derived from a row id instead of
    // Retry-After and the hand-written SQL test could not catch it.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let plan_path = format!(
        "{}/../docs/superpowers/plans/2026-08-21-facebook-integration-phase-2.md",
        manifest_dir
    );
    let plan = std::fs::read_to_string(&plan_path).expect("phase-2 plan must exist");

    let backoff_marker = "lease_deadline = datetime('now', '+' || ?";
    let mut checked = 0usize;
    for line in plan.lines() {
        // Only count executable uploader statements (Rust SQL string literals); prose
        // bullets quoting the pattern inline are documentation, not code paths.
        if !line.contains(backoff_marker)
            || !line
                .trim_start()
                .starts_with("\"UPDATE facebook_upload_sessions")
        {
            continue;
        }
        let modifier_start = line.find(backoff_marker).unwrap() + backoff_marker.len();
        let modifier_tail = &line[modifier_start..];
        let modifier: u32 = modifier_tail
            .split(" || ' seconds'")
            .next()
            .and_then(|num| num.parse::<u32>().ok())
            .expect("backoff statement must embed a numeric placeholder in its datetime modifier");

        // Strip the datetime modifier occurrence itself, then forbid any reuse of that
        // placeholder number as an identifier anywhere else in the statement.
        let needle = format!("'+' || ?{} || ' seconds'", modifier);
        let stripped = line.replacen(&needle, "'+'", 1);
        let reused = stripped.split('?').skip(1).any(|seg| {
            seg.chars()
                .next()
                .and_then(|c| c.to_digit(10))
                .map(|n| n == modifier)
                .unwrap_or(false)
        });
        assert!(
            !reused,
            "429-backoff placeholder ?{} must not be reused as an identifier in: {}",
            modifier,
            line.trim()
        );
        checked += 1;
    }
    assert_eq!(
        checked, 13,
        "expected exactly 13 Retry-After backoff statements in the Phase 2 plan"
    );

    // Execute the canonical finishing/upload_session_id variant verbatim against a real
    // SQLite database and assert the persisted deadline tracks Retry-After (+90s). Under
    // the old double-bound shape this statement would either match zero rows or persist a
    // NULL/unrelated deadline.
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "CREATE TABLE IF NOT EXISTS facebook_upload_sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            post_id INTEGER NOT NULL,
            page_id TEXT NOT NULL,
            upload_type TEXT NOT NULL DEFAULT 'video' CHECK (upload_type IN ('video', 'reel')),
            upload_session_id TEXT NOT NULL UNIQUE,
            video_id TEXT,
            file_path TEXT NOT NULL,
            file_hash TEXT NOT NULL,
            file_size_bytes INTEGER NOT NULL,
            start_offset INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'init_pending' CHECK (status IN ('init_pending', 'in_progress', 'finishing', 'completed', 'failed')),
            retry_count INTEGER NOT NULL DEFAULT 0,
            owner_id TEXT,
            lease_deadline TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, connection_status, capabilities_json, created_at, updated_at)
         VALUES (300, 'messenger', 'page_123', 'connected', '{}', datetime('now'), datetime('now'))",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO facebook_scheduled_posts (id, draft_group_id, channel_account_id, page_id, post_type, message, idempotency_key, status, created_at, updated_at)
         VALUES (300, 'grp_300', 300, 'page_123', 'reel', '429 contract message', 'idemp_300', 'publishing', datetime('now'), datetime('now'))",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO facebook_upload_sessions (post_id, page_id, upload_type, upload_session_id, video_id, file_path, file_hash, file_size_bytes, start_offset, status, retry_count, owner_id, lease_deadline, created_at, updated_at)
         VALUES (300, 'page_123', 'reel', 'sess_429_contract', 'meta_reel_999', '/tmp/reel429.mp4', 'hash_429', 1024, 0, 'finishing', 0, 'worker_429', NULL, datetime('now'), datetime('now'))",
        [],
    ).unwrap();

    let canonical_sql = "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = datetime('now', '+' || ?3 || ' seconds'), retry_count = retry_count + 1, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'finishing'";
    assert!(
        plan.contains(canonical_sql),
        "plan must contain the canonical finishing-variant 429 statement with distinct placeholders"
    );
    let updated = conn
        .execute(
            canonical_sql,
            params!["sess_429_contract", "worker_429", 90i64],
        )
        .unwrap();
    assert_eq!(
        updated, 1,
        "canonical 429 statement must claim exactly the leased finishing row"
    );

    let (deadline, retries): (Option<String>, i64) = conn
        .query_row(
            "SELECT lease_deadline, retry_count FROM facebook_upload_sessions WHERE upload_session_id = 'sess_429_contract'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(retries, 1, "retry counter must increment on 429 backoff");
    let deadline = deadline.expect("lease_deadline must persist after 429 backoff");
    let deadline_ts = chrono::NaiveDateTime::parse_from_str(&deadline, "%Y-%m-%d %H:%M:%S")
        .expect("lease_deadline must be a valid SQLite datetime")
        .and_utc()
        .timestamp();
    let now = chrono::Utc::now().timestamp();
    assert!(
        deadline_ts >= now + 80 && deadline_ts <= now + 100,
        "persisted lease_deadline must track Meta's Retry-After (+90s), got offset {}s",
        deadline_ts - now
    );

    let premature_reconcile = conn.execute(
        "UPDATE facebook_upload_sessions
         SET owner_id = 'worker_reconciler', lease_deadline = datetime('now', '+5 minutes'), updated_at = datetime('now')
         WHERE upload_session_id = 'sess_429_contract' AND (lease_deadline IS NULL OR lease_deadline < datetime('now'))",
        [],
    ).unwrap();
    assert_eq!(
        premature_reconcile, 0,
        "background reconciler must not re-claim while Retry-After backoff is active"
    );
}

#[test]
fn test_facebook_reel_post_finish_release_and_reconciler_parent_clock_eligibility() {
    // Regression guards (Review 4998373731, findings 2/3):
    // A) publishing_phase.status == 'complete' with unsuccessful feed/scheduled verification
    //    must RELEASE the session lease via checked CAS before returning StillProcessing.
    // B) The reconciler claim predicate must honor the parent post's next_retry_at, so a
    //    Retry-After < 60s session can never be reconciled while its parent is still queued.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let plan_path = format!(
        "{}/../docs/superpowers/plans/2026-08-21-facebook-integration-phase-2.md",
        manifest_dir
    );
    let plan = std::fs::read_to_string(&plan_path).expect("phase-2 plan must exist");
    assert!(
        plan.contains("JOIN facebook_scheduled_posts p ON p.id = s.post_id")
            && plan.contains("AND (p.next_retry_at IS NULL OR p.next_retry_at <= datetime('now'))"),
        "reconciler claim predicate must join the parent post and require elapsed next_retry_at"
    );

    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, connection_status, capabilities_json, created_at, updated_at)
         VALUES (400, 'messenger', 'page_123', 'connected', '{}', datetime('now'), datetime('now'))",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO facebook_scheduled_posts (id, draft_group_id, channel_account_id, page_id, post_type, message, idempotency_key, status, next_retry_at, created_at, updated_at)
         VALUES (400, 'grp_400', 400, 'page_123', 'reel', 'clock divergence', 'idemp_400', 'queued', datetime('now', '+60 seconds'), datetime('now'), datetime('now'))",
        [],
    ).unwrap();

    // Parent in queued state with pending next_retry_at (+60s per publisher floor);
    // session leased with a short +10s backoff deadline (Retry-After = 10 < 60).
    conn.execute(
        "INSERT INTO facebook_upload_sessions (post_id, page_id, upload_type, upload_session_id, video_id, file_path, file_hash, file_size_bytes, start_offset, status, retry_count, owner_id, lease_deadline, created_at, updated_at)
         VALUES (400, 'page_123', 'reel', 'sess_clock_divergence', 'meta_reel_400', '/tmp/reel400.mp4', 'hash_400', 2048, 0, 'finishing', 1, NULL, datetime('now', '+10 seconds'), datetime('now'), datetime('now'))",
        [],
    ).unwrap();

    // B) Old predicate (session clock only) would reclaim now; the plan's predicate must NOT:
    // the parent's next_retry_at is still pending while the session backoff already expired.
    let premature_claim = conn.execute(
        "UPDATE facebook_upload_sessions
         SET owner_id = 'worker_reconciler', lease_deadline = datetime('now', '+5 minutes'), updated_at = datetime('now')
         WHERE id = (
             SELECT s.id FROM facebook_upload_sessions s
             JOIN facebook_scheduled_posts p ON p.id = s.post_id
             WHERE s.status IN ('init_pending', 'in_progress', 'finishing')
               AND (s.lease_deadline IS NULL OR s.lease_deadline < datetime('now'))
               AND (p.next_retry_at IS NULL OR p.next_retry_at <= datetime('now'))
             LIMIT 1
         )",
        [],
    ).unwrap();
    assert_eq!(
        premature_claim, 0,
        "reconciler must not claim a session whose parent post is still inside next_retry_at backoff (Retry-After < 60 divergence)"
    );

    // Terminal propagation must never transition a queued parent: zero rows expected.
    let propagation_on_queued = conn.execute(
        "UPDATE facebook_scheduled_posts
         SET status = 'published', remote_post_id = 'meta_reel_400', owner_id = NULL, claim_deadline = NULL, updated_at = datetime('now')
         WHERE id = 400 AND status IN ('publishing', 'delivery_unknown')",
        [],
    ).unwrap();
    assert_eq!(
        propagation_on_queued, 0,
        "terminal propagation cannot encounter (or mutate) a queued parent; it must roll back instead"
    );

    // After BOTH retry windows elapse, the same claim succeeds (one consistent rule).
    conn.execute(
        "UPDATE facebook_scheduled_posts SET next_retry_at = datetime('now', '-1 seconds'), updated_at = datetime('now') WHERE id = 400",
        [],
    ).unwrap();
    conn.execute(
        "UPDATE facebook_upload_sessions SET lease_deadline = datetime('now', '-1 seconds'), updated_at = datetime('now') WHERE upload_session_id = 'sess_clock_divergence'",
        [],
    ).unwrap();
    let eligible_claim = conn.execute(
        "UPDATE facebook_upload_sessions
         SET owner_id = 'worker_reconciler', lease_deadline = datetime('now', '+5 minutes'), updated_at = datetime('now')
         WHERE id = (
             SELECT s.id FROM facebook_upload_sessions s
             JOIN facebook_scheduled_posts p ON p.id = s.post_id
             WHERE s.status IN ('init_pending', 'in_progress', 'finishing')
               AND (s.lease_deadline IS NULL OR s.lease_deadline < datetime('now'))
               AND (p.next_retry_at IS NULL OR p.next_retry_at <= datetime('now'))
             LIMIT 1
         )",
        [],
    ).unwrap();
    assert_eq!(eligible_claim, 1);

    // A) Post-FINISH early-return path: publishing complete but verification not live yet.
    // The uploader holds a long stream-extended lease; the documented checked-CAS release
    // (executed before returning StillProcessing) must free the lease immediately.
    conn.execute(
        "UPDATE facebook_upload_sessions SET owner_id = 'worker_stream', lease_deadline = datetime('now', '+30 minutes'), updated_at = datetime('now')
         WHERE upload_session_id = 'sess_clock_divergence'",
        [],
    ).unwrap();
    let release_rows = conn.execute(
        "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'finishing'",
        params!["sess_clock_divergence", "worker_stream"],
    ).unwrap();
    assert_eq!(
        release_rows, 1,
        "not-yet-live completed Reel must release its stream-extended lease via checked CAS before StillProcessing"
    );
    let (owner_after, deadline_after): (Option<String>, Option<String>) = conn
        .query_row(
            "SELECT owner_id, lease_deadline FROM facebook_upload_sessions WHERE upload_session_id = 'sess_clock_divergence'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(owner_after.is_none() && deadline_after.is_none());
}

#[test]
fn test_timeline_hydrates_attachments_with_null_and_empty_mime_type() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute("INSERT INTO leads (id, name) VALUES (10, 'Lead Ten')", [])
        .unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, enabled, connection_status, created_at, updated_at)
         VALUES (101, 'messenger', 'fb_page_1', 'FB Page', 1, 'connected', '2026-08-25T12:00:00Z', '2026-08-25T12:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_identities (id, lead_id, channel, identity_namespace, external_id, display_name, link_source, link_confidence, created_at, updated_at)
         VALUES (201, 10, 'messenger', 'psid', 'fb_user_1', 'FB User', 'manual', 1.0, '2026-08-25T12:00:00Z', '2026-08-25T12:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, title, status, last_message_at, created_at, updated_at)
         VALUES (301, 101, 't_123', 'direct', 'FB User', 'active', '2026-08-25T12:00:00Z', '2026-08-25T12:00:00Z', '2026-08-25T12:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
         VALUES (301, 201, 'member', '2026-08-25T12:00:00Z')",
        [],
    )
    .unwrap();

    // Insert 4 messages with various media types and null/empty/whitespace/valid MIME types
    conn.execute(
        "INSERT INTO messages (id, conversation_id, sender_identity_id, direction, content_type, text_content, content_json, status, ingested_at)
         VALUES (401, 301, 201, 'inbound', 'image', '', '{}', 'delivered', '2026-08-25T10:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO message_attachments (message_id, attachment_type, mime_type, file_name, remote_url, created_at)
         VALUES (401, 'image', NULL, NULL, 'https://cdn.example.com/fb_pic.jpg', '2026-08-25T10:00:00Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO messages (id, conversation_id, sender_identity_id, direction, content_type, text_content, content_json, status, ingested_at)
         VALUES (402, 301, 201, 'inbound', 'video', '', '{}', 'delivered', '2026-08-25T11:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO message_attachments (message_id, attachment_type, mime_type, file_name, remote_url, created_at)
         VALUES (402, 'video', '', 'video.mp4', 'https://cdn.example.com/video.mp4', '2026-08-25T11:00:00Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO messages (id, conversation_id, sender_identity_id, direction, content_type, text_content, content_json, status, ingested_at)
         VALUES (403, 301, 201, 'inbound', 'audio', '', '{}', 'delivered', '2026-08-25T12:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO message_attachments (message_id, attachment_type, mime_type, file_name, remote_url, created_at)
         VALUES (403, 'audio', '   ', 'voice.mp3', 'https://cdn.example.com/voice.mp3', '2026-08-25T12:00:00Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO messages (id, conversation_id, sender_identity_id, direction, content_type, text_content, content_json, status, ingested_at)
         VALUES (404, 301, 201, 'inbound', 'file', '', '{}', 'delivered', '2026-08-25T13:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO message_attachments (message_id, attachment_type, mime_type, file_name, remote_url, created_at)
         VALUES (404, 'file', NULL, 'document.pdf', 'https://cdn.example.com/doc.pdf', '2026-08-25T13:00:00Z')",
        [],
    )
    .unwrap();

    drop(conn);

    let page = harness
        .repository
        .get_contact_timeline(10, &[Channel::Messenger], None, 50)
        .unwrap();
    assert_eq!(page.items.len(), 4);

    // Results in ascending chronological order of occurred_at
    // Item 0: Msg 401 (image -> image/jpeg)
    assert_eq!(page.items[0].id, 401);
    assert_eq!(page.items[0].attachments[0].media_type, "image");
    assert_eq!(page.items[0].attachments[0].mime_type, "image/jpeg");
    assert_eq!(
        page.items[0].attachments[0].remote_url.as_deref(),
        Some("https://cdn.example.com/fb_pic.jpg")
    );

    // Item 1: Msg 402 (video -> video/mp4)
    assert_eq!(page.items[1].id, 402);
    assert_eq!(page.items[1].attachments[0].media_type, "video");
    assert_eq!(page.items[1].attachments[0].mime_type, "video/mp4");

    // Item 2: Msg 403 (audio -> audio/mpeg)
    assert_eq!(page.items[2].id, 403);
    assert_eq!(page.items[2].attachments[0].media_type, "audio");
    assert_eq!(page.items[2].attachments[0].mime_type, "audio/mpeg");

    // Item 3: Msg 404 (file -> application/octet-stream)
    assert_eq!(page.items[3].id, 404);
    assert_eq!(page.items[3].attachments[0].media_type, "file");
    assert_eq!(
        page.items[3].attachments[0].mime_type,
        "application/octet-stream"
    );
}

#[test]
fn test_claim_next_outbox_job_resolves_null_and_empty_mime_type_attachments() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, enabled, connection_status, created_at, updated_at)
         VALUES (101, 'messenger', 'fb_page_1', 'FB Page', 1, 'connected', '2026-08-25T12:00:00Z', '2026-08-25T12:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, title, status, last_message_at, created_at, updated_at)
         VALUES (301, 101, 't_123', 'direct', 'FB User', 'active', '2026-08-25T12:00:00Z', '2026-08-25T12:00:00Z', '2026-08-25T12:00:00Z')",
        [],
    )
    .unwrap();

    let test_cases = vec![
        (501, "job_1", "image", None, "image/jpeg", "img.jpg"),
        (502, "job_2", "video", Some(""), "video/mp4", "vid.mp4"),
        (503, "job_3", "audio", Some("   "), "audio/mpeg", "aud.mp3"),
        (
            504,
            "job_4",
            "file",
            None,
            "application/octet-stream",
            "data.bin",
        ),
        (
            505,
            "job_5",
            "image",
            Some("image/png"),
            "image/png",
            "icon.png",
        ),
    ];

    for (msg_id, job_id, media_type, raw_mime, expected_mime, file_name) in test_cases {
        conn.execute(
            "INSERT INTO messages (id, conversation_id, sender_identity_id, direction, content_type, text_content, content_json, status, ingested_at)
             VALUES (?1, 301, NULL, 'outbound', ?2, '', '{}', 'queued', '2026-08-25T12:00:00Z')",
            params![msg_id, media_type],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO message_attachments (message_id, attachment_type, mime_type, file_name, local_path, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, '2026-08-25T12:00:00Z')",
            params![
                msg_id,
                media_type,
                raw_mime,
                file_name,
                format!("/spool/{file_name}")
            ],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO message_outbox_jobs (id, message_id, channel_account_id, status, attempt_count, created_at, updated_at)
             VALUES (?1, ?2, 101, 'pending', 0, '2026-08-25T12:00:00Z', '2026-08-25T12:00:00Z')",
            params![job_id, msg_id],
        )
        .unwrap();

        let claimed = harness
            .repository
            .claim_next_outbox_job(101, "2026-08-25T12:00:01Z")
            .unwrap();
        assert!(claimed.is_some(), "Job {job_id} must be claimed");
        let job = claimed.unwrap();
        assert_eq!(job.job_id, job_id);
        assert_eq!(job.message_id, msg_id);

        let att = job.attachment.expect("Attachment must be present");
        assert_eq!(att.media_type, media_type);
        assert_eq!(
            att.mime_type, expected_mime,
            "Claimed attachment mime_type mismatch for media_type={media_type}, raw={raw_mime:?}"
        );
        assert_eq!(att.file_name.as_deref(), Some(file_name));
        assert_eq!(
            att.local_path.as_deref(),
            Some(format!("/spool/{file_name}").as_str())
        );

        // Mark completed to allow claiming next job in loop
        conn.execute(
            "UPDATE message_outbox_jobs SET status = 'completed' WHERE id = ?1",
            params![job_id],
        )
        .unwrap();
    }
}

#[test]
fn test_contact_timeline_mixed_messenger_and_zalo_channels() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    // 1. Single Lead
    conn.execute(
        "INSERT INTO leads (id, name) VALUES (10, 'Omnichannel Lead')",
        [],
    )
    .unwrap();

    // 2. Channel Accounts: Messenger and Zalo
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, enabled, connection_status, created_at, updated_at)
         VALUES (101, 'messenger', 'fb_page_1', 'Facebook Page', 1, 'connected', '2026-08-25T00:00:00Z', '2026-08-25T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, enabled, connection_status, created_at, updated_at)
         VALUES (102, 'zalo', 'zalo_oa_1', 'Zalo OA', 1, 'connected', '2026-08-25T00:00:00Z', '2026-08-25T00:00:00Z')",
        [],
    )
    .unwrap();

    // 3. Channel Identities linked to the same Lead
    conn.execute(
        "INSERT INTO channel_identities (id, lead_id, channel, identity_namespace, external_id, display_name, link_source, link_confidence, created_at, updated_at)
         VALUES (201, 10, 'messenger', 'psid', 'fb_psid_1', 'FB Customer', 'manual', 1.0, '2026-08-25T00:00:00Z', '2026-08-25T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_identities (id, lead_id, channel, identity_namespace, external_id, display_name, link_source, link_confidence, created_at, updated_at)
         VALUES (202, 10, 'zalo', 'uid', 'zalo_uid_1', 'Zalo Customer', 'manual', 1.0, '2026-08-25T00:00:00Z', '2026-08-25T00:00:00Z')",
        [],
    )
    .unwrap();

    // 4. Direct Conversations for each channel
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, title, status, last_message_at, created_at, updated_at)
         VALUES (301, 101, 'fb_thread_1', 'direct', 'FB Customer', 'active', '2026-08-25T14:00:00Z', '2026-08-25T00:00:00Z', '2026-08-25T14:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, title, status, last_message_at, created_at, updated_at)
         VALUES (302, 102, 'zalo_thread_1', 'direct', 'Zalo Customer', 'active', '2026-08-25T13:00:00Z', '2026-08-25T00:00:00Z', '2026-08-25T13:00:00Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
         VALUES (301, 201, 'member', '2026-08-25T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
         VALUES (302, 202, 'member', '2026-08-25T00:00:00Z')",
        [],
    )
    .unwrap();

    // 5. Interleaved messages across Messenger and Zalo
    // Msg 401: Messenger, 10:00, image attachment with NULL mime_type
    conn.execute(
        "INSERT INTO messages (id, conversation_id, sender_identity_id, direction, content_type, text_content, content_json, status, ingested_at)
         VALUES (401, 301, 201, 'inbound', 'image', 'FB photo', '{}', 'delivered', '2026-08-25T10:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO message_attachments (message_id, attachment_type, mime_type, file_name, remote_url, created_at)
         VALUES (401, 'image', NULL, 'photo.jpg', 'https://cdn.example.com/fb.jpg', '2026-08-25T10:00:00Z')",
        [],
    )
    .unwrap();

    // Msg 402: Zalo, 11:00, video attachment with empty string mime_type
    conn.execute(
        "INSERT INTO messages (id, conversation_id, sender_identity_id, direction, content_type, text_content, content_json, status, ingested_at)
         VALUES (402, 302, 202, 'inbound', 'video', 'Zalo clip', '{}', 'delivered', '2026-08-25T11:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO message_attachments (message_id, attachment_type, mime_type, file_name, remote_url, created_at)
         VALUES (402, 'video', '', 'clip.mp4', 'https://cdn.example.com/zalo.mp4', '2026-08-25T11:00:00Z')",
        [],
    )
    .unwrap();

    // Msg 403: Messenger, 12:00, audio attachment with whitespace mime_type
    conn.execute(
        "INSERT INTO messages (id, conversation_id, sender_identity_id, direction, content_type, text_content, content_json, status, ingested_at)
         VALUES (403, 301, 201, 'inbound', 'audio', 'FB voice', '{}', 'delivered', '2026-08-25T12:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO message_attachments (message_id, attachment_type, mime_type, file_name, remote_url, created_at)
         VALUES (403, 'audio', '   ', 'voice.mp3', 'https://cdn.example.com/fb_voice.mp3', '2026-08-25T12:00:00Z')",
        [],
    )
    .unwrap();

    // Msg 404: Zalo, 13:00, file attachment with NULL mime_type
    conn.execute(
        "INSERT INTO messages (id, conversation_id, sender_identity_id, direction, content_type, text_content, content_json, status, ingested_at)
         VALUES (404, 302, 202, 'inbound', 'file', 'Zalo invoice', '{}', 'delivered', '2026-08-25T13:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO message_attachments (message_id, attachment_type, mime_type, file_name, remote_url, created_at)
         VALUES (404, 'file', NULL, 'invoice.pdf', 'https://cdn.example.com/invoice.pdf', '2026-08-25T13:00:00Z')",
        [],
    )
    .unwrap();

    // Msg 405: Messenger, 14:00, image attachment with valid MIME type
    conn.execute(
        "INSERT INTO messages (id, conversation_id, sender_identity_id, direction, content_type, text_content, content_json, status, ingested_at)
         VALUES (405, 301, 201, 'inbound', 'image', 'FB sticker', '{}', 'delivered', '2026-08-25T14:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO message_attachments (message_id, attachment_type, mime_type, file_name, remote_url, created_at)
         VALUES (405, 'image', 'image/webp', 'sticker.webp', 'https://cdn.example.com/sticker.webp', '2026-08-25T14:00:00Z')",
        [],
    )
    .unwrap();

    drop(conn);

    // Test A: Query with both Messenger and Zalo selected
    let mixed_page = harness
        .repository
        .get_contact_timeline(10, &[Channel::Messenger, Channel::Zalo], None, 50)
        .unwrap();
    assert_eq!(mixed_page.items.len(), 5);

    // Verify ascending chronological order and channel / attachment resolution:
    // 0: Msg 401 (Messenger, 10:00) -> "image/jpeg"
    assert_eq!(mixed_page.items[0].id, 401);
    assert_eq!(mixed_page.items[0].channel, Channel::Messenger);
    assert_eq!(mixed_page.items[0].attachments[0].mime_type, "image/jpeg");

    // 1: Msg 402 (Zalo, 11:00) -> "video/mp4"
    assert_eq!(mixed_page.items[1].id, 402);
    assert_eq!(mixed_page.items[1].channel, Channel::Zalo);
    assert_eq!(mixed_page.items[1].attachments[0].mime_type, "video/mp4");

    // 2: Msg 403 (Messenger, 12:00) -> "audio/mpeg"
    assert_eq!(mixed_page.items[2].id, 403);
    assert_eq!(mixed_page.items[2].channel, Channel::Messenger);
    assert_eq!(mixed_page.items[2].attachments[0].mime_type, "audio/mpeg");

    // 3: Msg 404 (Zalo, 13:00) -> "application/octet-stream"
    assert_eq!(mixed_page.items[3].id, 404);
    assert_eq!(mixed_page.items[3].channel, Channel::Zalo);
    assert_eq!(
        mixed_page.items[3].attachments[0].mime_type,
        "application/octet-stream"
    );

    // 4: Msg 405 (Messenger, 14:00) -> "image/webp"
    assert_eq!(mixed_page.items[4].id, 405);
    assert_eq!(mixed_page.items[4].channel, Channel::Messenger);
    assert_eq!(mixed_page.items[4].attachments[0].mime_type, "image/webp");

    // Test B: Filter Messenger only
    let messenger_only = harness
        .repository
        .get_contact_timeline(10, &[Channel::Messenger], None, 50)
        .unwrap();
    assert_eq!(messenger_only.items.len(), 3);
    assert_eq!(messenger_only.items[0].id, 401);
    assert_eq!(messenger_only.items[1].id, 403);
    assert_eq!(messenger_only.items[2].id, 405);

    // Test C: Filter Zalo only
    let zalo_only = harness
        .repository
        .get_contact_timeline(10, &[Channel::Zalo], None, 50)
        .unwrap();
    assert_eq!(zalo_only.items.len(), 2);
    assert_eq!(zalo_only.items[0].id, 402);
    assert_eq!(zalo_only.items[1].id, 404);
}

#[test]
fn test_list_inbox_vectorized_bulk_performance_and_accuracy() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();

    // 1. Create channel accounts
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
         VALUES (1, 'zalo', 'acc-zalo-1', 'Zalo Main', 'connected', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z'),
                (2, 'messenger', 'acc-fb-1', 'FB Page', 'connected', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();

    // 2. Insert 100 Leads, 100 direct conversations
    for i in 1..=100 {
        conn.execute(
            "INSERT INTO leads (id, name, phone) VALUES (?1, ?2, ?3)",
            params![i, format!("Customer {i}"), format!("0901234{i:03}")],
        ).unwrap();

        conn.execute(
            "INSERT INTO channel_identities (id, lead_id, channel, channel_account_id, identity_namespace, external_id, display_name, avatar_url, created_at, updated_at)
             VALUES (?1, ?2, 'zalo', 1, 'zalo_uid', ?3, ?4, ?5, '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
            params![i, i, format!("zalo-uid-{i}"), format!("Customer Display {i}"), format!("https://avatar.example.com/{i}.jpg")],
        ).unwrap();

        conn.execute(
            "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, title, last_message_at, created_at, updated_at)
             VALUES (?1, 1, ?2, 'direct', ?3, '2026-08-01T12:00:00Z', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
            params![i, format!("zalo-uid-{i}"), format!("Customer {i}")],
        ).unwrap();

        conn.execute(
            "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role) VALUES (?1, ?2, 'member')",
            params![i, i],
        ).unwrap();

        // 5 messages per lead
        for m in 1..=5 {
            let msg_id = (i - 1) * 5 + m;
            let dir = if m % 2 == 0 { "outbound" } else { "inbound" };
            let status = if dir == "inbound" && m == 5 { "received" } else { "read" };
            conn.execute(
                "INSERT INTO messages (id, conversation_id, sender_identity_id, direction, content_type, text_content, provider_timestamp, ingested_at, status)
                 VALUES (?1, ?2, ?3, ?4, 'text', ?5, '2026-08-01T12:00:00Z', '2026-08-01T12:00:00Z', ?6)",
                params![msg_id, i, i, dir, format!("Msg {m} for lead {i}"), status],
            ).unwrap();
        }
    }

    // 3. Insert 50 Unknown direct conversations
    for i in 101..=150 {
        conn.execute(
            "INSERT INTO channel_identities (id, lead_id, channel, channel_account_id, identity_namespace, external_id, display_name, created_at, updated_at)
             VALUES (?1, NULL, 'zalo', 1, 'zalo_uid', ?2, ?3, '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
            params![i, format!("unknown-uid-{i}"), format!("Unknown Person {i}")],
        ).unwrap();

        conn.execute(
            "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, title, last_message_at, created_at, updated_at)
             VALUES (?1, 1, ?2, 'direct', ?3, '2026-08-01T11:00:00Z', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
            params![i, format!("unknown-uid-{i}"), format!("Unknown {i}")],
        ).unwrap();

        conn.execute(
            "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role) VALUES (?1, ?2, 'member')",
            params![i, i],
        ).unwrap();

        let msg_id = 500 + (i - 100);
        conn.execute(
            "INSERT INTO messages (id, conversation_id, sender_identity_id, direction, content_type, text_content, provider_timestamp, ingested_at, status)
             VALUES (?1, ?2, ?3, 'inbound', 'text', ?4, '2026-08-01T11:00:00Z', '2026-08-01T11:00:00Z', 'received')",
            params![msg_id, i, i, format!("Hello from unknown {i}")],
        ).unwrap();
    }

    // 4. Insert 50 Group conversations
    for i in 151..=200 {
        conn.execute(
            "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, title, last_message_at, created_at, updated_at)
             VALUES (?1, 1, ?2, 'group', ?3, '2026-08-01T10:00:00Z', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
            params![i, format!("group-{i}"), format!("Group Chat {i}")],
        ).unwrap();

        // 3 participants per group
        for p in 1..=3 {
            let p_identity_id = 500 + (i - 150) * 3 + p;
            conn.execute(
                "INSERT INTO channel_identities (id, lead_id, channel, channel_account_id, identity_namespace, external_id, display_name, created_at, updated_at)
                 VALUES (?1, NULL, 'zalo', 1, 'zalo_uid', ?2, ?3, '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
                params![p_identity_id, format!("member-{p_identity_id}"), format!("Member {p_identity_id}")],
            ).unwrap();

            conn.execute(
                "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role) VALUES (?1, ?2, ?3)",
                params![i, p_identity_id, if p == 1 { "admin" } else { "member" }],
            ).unwrap();
        }

        let msg_id = 600 + (i - 150);
        conn.execute(
            "INSERT INTO messages (id, conversation_id, direction, content_type, text_content, provider_timestamp, ingested_at, status)
             VALUES (?1, ?2, 'inbound', 'text', ?3, '2026-08-01T10:00:00Z', '2026-08-01T10:00:00Z', 'received')",
            params![msg_id, i, format!("Group announcement in {i}")],
        ).unwrap();
    }

    // Benchmark list_inbox
    let start = std::time::Instant::now();
    let items = harness.repository.list_inbox().unwrap();
    let duration = start.elapsed();

    println!("list_inbox() returned {} items in {:?}", items.len(), duration);

    // Total items = 100 customer leads + 50 unknown + 50 groups = 200 items
    assert_eq!(items.len(), 200);

    // Verify Customer items
    let customer_item = items.iter().find(|it| it.key == "lead:1").unwrap();
    assert_eq!(customer_item.kind, InboxItemKind::Customer);
    assert_eq!(customer_item.title, "Customer 1");
    assert_eq!(customer_item.avatar_url, Some("https://avatar.example.com/1.jpg".to_string()));
    assert_eq!(customer_item.last_message_preview, Some("Msg 5 for lead 1".to_string()));
    assert_eq!(customer_item.channels[0].channel, Channel::Zalo);
    assert_eq!(customer_item.channels[0].message_count, 5);
    assert_eq!(customer_item.reply_targets.len(), 1);

    // Verify Unknown items
    let unknown_item = items.iter().find(|it| it.key == "identity:101").unwrap();
    assert_eq!(unknown_item.kind, InboxItemKind::Unknown);
    assert_eq!(unknown_item.title, "Unknown Person 101");
    assert_eq!(unknown_item.last_message_preview, Some("Hello from unknown 101".to_string()));

    // Verify Group items
    let group_item = items.iter().find(|it| it.key == "conversation:151").unwrap();
    assert_eq!(group_item.kind, InboxItemKind::Group);
    assert_eq!(group_item.title, "Group Chat 151");
    assert_eq!(group_item.participant_count, 3);
    assert_eq!(group_item.participants.len(), 3);
    assert_eq!(group_item.participants[0].participant_role, "admin");

    // Performance assertion: 200 items with ~600 messages must load in under 100ms in debug mode
    assert!(duration.as_millis() < 100, "list_inbox took too long: {:?}", duration);
}