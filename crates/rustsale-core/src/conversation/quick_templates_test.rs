use super::migration::migrate;
use super::quick_templates::{normalize_shortcut, QuickTemplateRepository};
use super::types::{
    CreateConversationQuickTemplateRequest, UpdateConversationQuickTemplateRequest,
};
use crate::conversation::error::ConversationErrorCode;
use crate::db::open_db_connection;

struct Harness {
    path: std::path::PathBuf,
    repo: QuickTemplateRepository,
}

impl Harness {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rustsale-quick-templates-test-{}.db",
            rand::random::<u64>()
        ));
        let conn = open_db_connection(path.to_str().unwrap()).unwrap();
        migrate(&conn).unwrap();
        drop(conn);
        Self {
            repo: QuickTemplateRepository::new(path.to_string_lossy().to_string()),
            path,
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[test]
fn test_shortcut_normalization() {
    assert_eq!(normalize_shortcut("/hello"), "hello");
    assert_eq!(normalize_shortcut("  ///GREETING  "), "greeting");
    assert_eq!(normalize_shortcut("  /Test_123  "), "test_123");
    assert_eq!(normalize_shortcut("simple"), "simple");
}

#[test]
fn test_create_and_get_quick_template() {
    let harness = Harness::new();
    let req = CreateConversationQuickTemplateRequest {
        shortcut: "/hello".into(),
        title: "Greeting".into(),
        content: "Hello there!".into(),
        category: Some("General".into()),
    };
    let created = harness.repo.create(req).unwrap();
    assert_eq!(created.shortcut, "hello");
    assert_eq!(created.title, "Greeting");
    assert_eq!(created.content, "Hello there!");
    assert_eq!(created.category, Some("General".into()));
    assert_eq!(created.usage_count, 0);
    assert!(created.last_used_at.is_none());

    let list = harness.repo.list(false).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, created.id);
}

#[test]
fn test_update_quick_template() {
    let harness = Harness::new();
    let req = CreateConversationQuickTemplateRequest {
        shortcut: "/hello".into(),
        title: "Greeting".into(),
        content: "Hello there!".into(),
        category: Some("General".into()),
    };
    let created = harness.repo.create(req).unwrap();

    let update_req = UpdateConversationQuickTemplateRequest {
        id: created.id,
        shortcut: "/hello_updated".into(),
        title: "Greeting Updated".into(),
        content: "Hello updated!".into(),
        category: Some("Updated".into()),
        is_active: true,
    };
    let updated = harness.repo.update(update_req).unwrap();
    assert_eq!(updated.shortcut, "hello_updated");
    assert_eq!(updated.title, "Greeting Updated");
    assert_eq!(updated.content, "Hello updated!");
    assert_eq!(updated.category, Some("Updated".into()));
}

#[test]
fn test_validation_bounds() {
    let harness = Harness::new();
    // Empty shortcut
    let err = harness
        .repo
        .create(CreateConversationQuickTemplateRequest {
            shortcut: "/".into(),
            title: "Title".into(),
            content: "Body".into(),
            category: None,
        })
        .unwrap_err();
    assert_eq!(err.code, ConversationErrorCode::InvalidRequest);

    // Shortcut > 32 chars
    let err = harness
        .repo
        .create(CreateConversationQuickTemplateRequest {
            shortcut: format!("/{}", "a".repeat(33)),
            title: "Title".into(),
            content: "Body".into(),
            category: None,
        })
        .unwrap_err();
    assert_eq!(err.code, ConversationErrorCode::InvalidRequest);

    // Title empty or > 100
    let err = harness
        .repo
        .create(CreateConversationQuickTemplateRequest {
            shortcut: "valid".into(),
            title: " ".into(),
            content: "Body".into(),
            category: None,
        })
        .unwrap_err();
    assert_eq!(err.code, ConversationErrorCode::InvalidRequest);

    let err = harness
        .repo
        .create(CreateConversationQuickTemplateRequest {
            shortcut: "valid".into(),
            title: "t".repeat(101),
            content: "Body".into(),
            category: None,
        })
        .unwrap_err();
    assert_eq!(err.code, ConversationErrorCode::InvalidRequest);

    // Body empty or > 4000
    let err = harness
        .repo
        .create(CreateConversationQuickTemplateRequest {
            shortcut: "valid".into(),
            title: "Title".into(),
            content: "".into(),
            category: None,
        })
        .unwrap_err();
    assert_eq!(err.code, ConversationErrorCode::InvalidRequest);

    let err = harness
        .repo
        .create(CreateConversationQuickTemplateRequest {
            shortcut: "valid".into(),
            title: "Title".into(),
            content: "b".repeat(4001),
            category: None,
        })
        .unwrap_err();
    assert_eq!(err.code, ConversationErrorCode::InvalidRequest);
}

#[test]
fn test_shortcut_conflict() {
    let harness = Harness::new();
    harness
        .repo
        .create(CreateConversationQuickTemplateRequest {
            shortcut: "/greet".into(),
            title: "T1".into(),
            content: "B1".into(),
            category: None,
        })
        .unwrap();

    let err = harness
        .repo
        .create(CreateConversationQuickTemplateRequest {
            shortcut: "GREET".into(),
            title: "T2".into(),
            content: "B2".into(),
            category: None,
        })
        .unwrap_err();
    assert_eq!(err.code, ConversationErrorCode::TemplateShortcutConflict);
}

#[test]
fn test_500_item_limit() {
    let harness = Harness::new();
    for i in 0..500 {
        harness
            .repo
            .create(CreateConversationQuickTemplateRequest {
                shortcut: format!("sc_{i}"),
                title: format!("Title {i}"),
                content: format!("Content {i}"),
                category: None,
            })
            .unwrap();
    }

    let err = harness
        .repo
        .create(CreateConversationQuickTemplateRequest {
            shortcut: "sc_over_limit".into(),
            title: "Over limit".into(),
            content: "Content".into(),
            category: None,
        })
        .unwrap_err();
    assert_eq!(err.code, ConversationErrorCode::TemplateLimitExceeded);
}

#[test]
fn test_active_filtering_and_deletion() {
    let harness = Harness::new();
    let t1 = harness
        .repo
        .create(CreateConversationQuickTemplateRequest {
            shortcut: "t1".into(),
            title: "T1".into(),
            content: "Content 1".into(),
            category: None,
        })
        .unwrap();
    let _t2 = harness
        .repo
        .create(CreateConversationQuickTemplateRequest {
            shortcut: "t2".into(),
            title: "T2".into(),
            content: "Content 2".into(),
            category: None,
        })
        .unwrap();

    harness
        .repo
        .update(UpdateConversationQuickTemplateRequest {
            id: t1.id,
            shortcut: t1.shortcut.clone(),
            title: t1.title.clone(),
            content: t1.content.clone(),
            category: t1.category.clone(),
            is_active: false,
        })
        .unwrap();

    let active = harness.repo.list(false).unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].shortcut, "t2");

    let all = harness.repo.list(true).unwrap();
    assert_eq!(all.len(), 2);

    harness.repo.delete(t1.id).unwrap();
    let after_delete = harness.repo.list(true).unwrap();
    assert_eq!(after_delete.len(), 1);
}

#[test]
fn test_mark_used() {
    let harness = Harness::new();
    let t1 = harness
        .repo
        .create(CreateConversationQuickTemplateRequest {
            shortcut: "t1".into(),
            title: "T1".into(),
            content: "Content 1".into(),
            category: None,
        })
        .unwrap();

    let updated = harness.repo.mark_used(t1.id).unwrap();
    assert_eq!(updated.usage_count, 1);
    assert!(updated.last_used_at.is_some());

    let updated_again = harness.repo.mark_used(t1.id).unwrap();
    assert_eq!(updated_again.usage_count, 2);
}

#[test]
fn test_search_ranking_buckets_and_ordering() {
    let harness = Harness::new();

    // 0: exact shortcut match -> shortcut="hello"
    let t_exact = harness
        .repo
        .create(CreateConversationQuickTemplateRequest {
            shortcut: "hello".into(),
            title: "Say Hi".into(),
            content: "Greeting".into(),
            category: None,
        })
        .unwrap();

    // 1: shortcut prefix match -> shortcut="hello_world"
    let t_sc_prefix = harness
        .repo
        .create(CreateConversationQuickTemplateRequest {
            shortcut: "hello_world".into(),
            title: "World Greeting".into(),
            content: "Greeting".into(),
            category: None,
        })
        .unwrap();

    // 2: title prefix match -> title="Hello Friend"
    let t_title_prefix = harness
        .repo
        .create(CreateConversationQuickTemplateRequest {
            shortcut: "friend".into(),
            title: "Hello Friend".into(),
            content: "Nice to meet you".into(),
            category: None,
        })
        .unwrap();

    // 3: category match -> category="hello"
    let t_category = harness
        .repo
        .create(CreateConversationQuickTemplateRequest {
            shortcut: "cat_item".into(),
            title: "Category Item".into(),
            content: "Some text".into(),
            category: Some("Hello".into()),
        })
        .unwrap();

    // 4: body substring match -> body="Special hello text"
    let t_body = harness
        .repo
        .create(CreateConversationQuickTemplateRequest {
            shortcut: "body_item".into(),
            title: "Body Item".into(),
            content: "Special hello text".into(),
            category: None,
        })
        .unwrap();

    let results = harness.repo.search("hello", 10).unwrap();
    assert_eq!(results.len(), 5);
    assert_eq!(results[0].id, t_exact.id);
    assert_eq!(results[1].id, t_sc_prefix.id);
    assert_eq!(results[2].id, t_title_prefix.id);
    assert_eq!(results[3].id, t_category.id);
    assert_eq!(results[4].id, t_body.id);
}
