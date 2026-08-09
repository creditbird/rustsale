use crate::conversation::error::ConversationErrorCode;
use crate::conversation::migration::migrate;
use crate::conversation::reactions::ReactionMutationRepository;
use crate::conversation::types::{ConversationReactionCode, SetConversationReactionRequest};
use crate::db::open_db_connection;
use std::path::PathBuf;

fn setup_test_repo() -> (PathBuf, ReactionMutationRepository) {
    let db_path = std::env::temp_dir().join(format!(
        "rustsale-reactions-test-{}.db",
        rand::random::<u64>()
    ));
    let conn = open_db_connection(db_path.to_str().unwrap()).unwrap();
    migrate(&conn).unwrap();

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

    let repo = ReactionMutationRepository::new(db_path.clone());
    (db_path, repo)
}

#[test]
fn begin_mutation_is_idempotent_by_client_id() {
    let (_path, repo) = setup_test_repo();
    let req = SetConversationReactionRequest {
        message_id: 10,
        reaction: ConversationReactionCode::Heart,
        active: true,
        client_mutation_id: "mut-1".to_string(),
    };

    let first = repo.begin(&req, 1).unwrap();
    assert_eq!(first.client_mutation_id, "mut-1");
    assert_eq!(first.message_id, 10);
    assert_eq!(first.reaction_code, "heart");
    assert!(!first.is_existing);

    let second = repo.begin(&req, 1).unwrap();
    assert_eq!(second.client_mutation_id, first.client_mutation_id);
    assert_eq!(second.message_id, first.message_id);
    assert_eq!(second.reaction_code, first.reaction_code);
    assert_eq!(second.active, first.active);
    assert_eq!(second.status, first.status);
    assert!(second.is_existing);
}

#[test]
fn same_client_id_with_different_payload_is_rejected() {
    let (_path, repo) = setup_test_repo();
    let req1 = SetConversationReactionRequest {
        message_id: 10,
        reaction: ConversationReactionCode::Heart,
        active: true,
        client_mutation_id: "mut-1".to_string(),
    };

    repo.begin(&req1, 1).unwrap();

    let req2 = SetConversationReactionRequest {
        message_id: 10,
        reaction: ConversationReactionCode::Like,
        active: true,
        client_mutation_id: "mut-1".to_string(),
    };

    let err = repo.begin(&req2, 1).unwrap_err();
    assert_eq!(err.code, ConversationErrorCode::InvalidRequest);
}

#[test]
fn successful_local_apply_upserts_self_actor_once() {
    let (path, repo) = setup_test_repo();
    let req = SetConversationReactionRequest {
        message_id: 10,
        reaction: ConversationReactionCode::Heart,
        active: true,
        client_mutation_id: "mut-1".to_string(),
    };

    repo.begin(&req, 1).unwrap();
    let groups = repo
        .succeed("mut-1", "self-uid", "2026-08-02T12:00:00Z")
        .unwrap();

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].reaction_icon, "❤️");
    assert_eq!(groups[0].count, 1);
    assert!(groups[0].includes_me);

    let groups2 = repo
        .succeed("mut-1", "self-uid", "2026-08-02T12:00:00Z")
        .unwrap();
    assert_eq!(groups2.len(), 1);
    assert_eq!(groups2[0].count, 1);

    let conn = open_db_connection(path.to_str().unwrap()).unwrap();
    let status: String = conn
        .query_row(
            "SELECT status FROM reaction_mutations WHERE client_mutation_id = 'mut-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "succeeded");
}

#[test]
fn successful_local_remove_matches_provider_reaction_alias() {
    let (path, repo) = setup_test_repo();
    let conn = open_db_connection(path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO message_reactions (message_id, actor_external_id, reaction, reacted_at, removed_at)
         VALUES (10, 'self-uid', '/-heart', '2026-08-02T11:00:00Z', NULL)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO message_reactions (message_id, actor_external_id, reaction, reacted_at, removed_at)
         VALUES (10, 'self-uid', 'Unknown', '2026-08-02T11:30:00Z', NULL)",
        [],
    )
    .unwrap();

    let request = SetConversationReactionRequest {
        message_id: 10,
        reaction: ConversationReactionCode::Heart,
        active: false,
        client_mutation_id: "mut-remove-heart".to_string(),
    };

    repo.begin(&request, 1).unwrap();
    let groups = repo
        .succeed("mut-remove-heart", "self-uid", "2026-08-02T12:00:00Z")
        .unwrap();

    assert!(groups.is_empty());
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM message_reactions
             WHERE message_id = 10 AND actor_external_id = 'self-uid' AND reaction = '/-heart'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn multi_emoji_remove_hard_deletes_only_selected_reaction() {
    let (path, repo) = setup_test_repo();
    let conn = open_db_connection(path.to_str().unwrap()).unwrap();

    let req_heart = SetConversationReactionRequest {
        message_id: 10,
        reaction: ConversationReactionCode::Heart,
        active: true,
        client_mutation_id: "mut-heart".to_string(),
    };
    repo.begin(&req_heart, 1).unwrap();
    repo.succeed("mut-heart", "self-uid", "2026-08-02T12:00:00Z")
        .unwrap();

    let req_haha = SetConversationReactionRequest {
        message_id: 10,
        reaction: ConversationReactionCode::Haha,
        active: true,
        client_mutation_id: "mut-haha".to_string(),
    };
    repo.begin(&req_haha, 1).unwrap();
    repo.succeed("mut-haha", "self-uid", "2026-08-02T12:01:00Z")
        .unwrap();

    let req_remove_heart = SetConversationReactionRequest {
        message_id: 10,
        reaction: ConversationReactionCode::Heart,
        active: false,
        client_mutation_id: "mut-remove-heart-2".to_string(),
    };
    repo.begin(&req_remove_heart, 1).unwrap();
    repo.succeed("mut-remove-heart-2", "self-uid", "2026-08-02T12:02:00Z")
        .unwrap();

    let stored: Vec<String> = conn
        .prepare(
            "SELECT reaction FROM message_reactions
             WHERE message_id = 10 AND actor_external_id = 'self-uid'
             ORDER BY reaction",
        )
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(stored, vec!["haha".to_string()]);
}

#[test]
fn echo_cache_is_scoped_by_account_and_consumes_once() {
    use crate::conversation::echo_cache::ReactionEchoCache;

    let cache = ReactionEchoCache::new();
    cache.mark_expected(1, "zalo-msg-123", "heart", "self-uid");

    assert!(!cache.consume_if_expected(2, "zalo-msg-123", "heart", "self-uid"));
    assert!(cache.consume_if_expected(1, "zalo-msg-123", "heart", "self-uid"));
    assert!(!cache.consume_if_expected(1, "zalo-msg-123", "heart", "self-uid"));
}

#[test]
fn echo_cache_clear_removes_all_failed_request_aliases() {
    use crate::conversation::echo_cache::ReactionEchoCache;

    let cache = ReactionEchoCache::new();
    cache.mark_expected(1, "server-id", "heart", "self-uid");
    cache.mark_expected(1, "client-id", "heart", "self-uid");

    cache.clear_expected(1, "server-id", "heart", "self-uid");
    cache.clear_expected(1, "client-id", "heart", "self-uid");

    assert!(!cache.consume_if_expected(1, "server-id", "heart", "self-uid"));
    assert!(!cache.consume_if_expected(1, "client-id", "heart", "self-uid"));
}

#[test]
fn startup_running_mutations_become_delivery_unknown() {
    let (path, repo) = setup_test_repo();
    let conn = open_db_connection(path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO reaction_mutations (client_mutation_id, message_id, reaction_code, active, status, created_at, updated_at)
         VALUES ('mut-r1', 10, 'heart', 1, 'running', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z'),
                ('mut-r2', 10, 'like', 1, 'running', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z'),
                ('mut-s1', 10, 'haha', 1, 'succeeded', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z')",
        [],
    ).unwrap();

    let updated = repo
        .mark_running_delivery_unknown("2026-08-02T12:00:00Z")
        .unwrap();
    assert_eq!(updated, 2);

    let r1_status: String = conn
        .query_row(
            "SELECT status FROM reaction_mutations WHERE client_mutation_id = 'mut-r1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(r1_status, "delivery_unknown");

    let s1_status: String = conn
        .query_row(
            "SELECT status FROM reaction_mutations WHERE client_mutation_id = 'mut-s1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(s1_status, "succeeded");
}
