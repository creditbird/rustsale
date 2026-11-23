//! Faults found by the int6 QA that live in the core (#52). Each test names its item.

mod n3 {
    use crate::{commands, db, models};

    fn assert_not_found(result: Result<(), String>, what: &str, id: i64) {
        let error = result.expect_err("delete of a missing row must fail");
        assert_eq!(error, format!("{what} {id} not found"));
    }

    #[test]
    fn deleting_a_missing_template_or_category_is_not_found() {
        let conn = db::init_db(":memory:").unwrap();
        assert_not_found(
            commands::delete_category_internal(&conn, 777),
            "category",
            777,
        );
        assert_not_found(
            commands::delete_template_internal(&conn, 777),
            "template",
            777,
        );
        assert_not_found(
            commands::delete_email_template_internal(&conn, 777),
            "email template",
            777,
        );
        assert_not_found(
            commands::delete_outreach_template_internal(&conn, 777),
            "outreach template",
            777,
        );
        assert_not_found(
            commands::delete_template_folder_internal(&conn, 777),
            "template folder",
            777,
        );
        assert_not_found(
            commands::delete_smtp_server_internal(&conn, 777),
            "SMTP server",
            777,
        );
    }

    #[test]
    fn deleting_twice_succeeds_once_then_is_not_found() {
        let conn = db::init_db(":memory:").unwrap();
        let category = db::create_category(&conn, "Khach VIP").unwrap();
        commands::delete_category_internal(&conn, category).unwrap();
        assert_not_found(
            commands::delete_category_internal(&conn, category),
            "category",
            i64::from(category),
        );

        let input = models::CreateTemplateInput {
            folder_id: None,
            name: "hello".into(),
            channel: "zalo".into(),
            stage: None,
            subject: None,
            content: "hi".into(),
            media_url: None,
            ai_prompt: None,
            tags_json: None,
        };
        let template = db::create_template(&conn, &input).unwrap();
        commands::delete_template_internal(&conn, template).unwrap();
        assert_not_found(
            commands::delete_template_internal(&conn, template),
            "template",
            template,
        );
    }
}

/// N2 timing harness (ignored): `INT7_BENCH_DB=/tmp/.../rustsale.db cargo test --release
/// -p rustsale-core int7_core_test::n2_inbox_bench -- --ignored --nocapture`. Only ever point it
/// at a copy of the data: it opens the file read-write like the app does.
mod n2 {
    use crate::conversation::repository::ConversationRepository;
    use crate::paging::inbox::{page_inbox, InboxQuery};
    use std::time::Instant;

    use crate::conversation::commands_test::{direct_message, group_message, ServiceHarness};
    use crate::conversation::repository::NormalizedMessage;
    use crate::conversation::types::{MessageDirection, MessageStatus};

    fn at(mut message: NormalizedMessage, id: &str, ts: &str, text: &str) -> NormalizedMessage {
        message.provider_message_id = Some(id.to_string());
        message.provider_client_message_id = Some(format!("client-{id}"));
        message.provider_timestamp = Some(format!("2026-08-01T00:00:{ts}.000Z"));
        message.ingested_at = format!("2026-08-01T00:00:{ts}.500Z");
        message.text_content = Some(text.to_string());
        message.content = serde_json::json!({ "text": text });
        message
    }

    fn outbound(mut message: NormalizedMessage) -> NormalizedMessage {
        message.direction = MessageDirection::Outbound;
        message.status = MessageStatus::Sent;
        message
    }

    #[test]
    fn group_and_customer_rows_keep_counts_unread_and_last_message() {
        let harness = ServiceHarness::new();
        harness.insert_lead(7, "Lead Seven");
        let repository = &harness.repository;
        for message in [
            at(group_message("acc", "g1", "x"), "g1-a", "01", "one"),
            outbound(at(group_message("acc", "g1", "x"), "g1-b", "02", "mine")),
            at(group_message("acc", "g1", "x"), "g1-c", "03", "three"),
            at(group_message("acc", "g1", "x"), "g1-d", "04", "last"),
        ] {
            repository.apply_normalized_message(&message).unwrap();
        }
        let linked = repository
            .apply_normalized_message(&at(direct_message("acc", "x"), "d-a", "05", "hi"))
            .unwrap();
        repository
            .link_identity(linked.identity_id.unwrap(), 7)
            .unwrap();
        repository
            .apply_normalized_message(&outbound(at(direct_message("acc", "x"), "d-b", "06", "re")))
            .unwrap();

        let rows = harness.service.list_inbox().unwrap();
        let group = rows
            .iter()
            .find(|row| row.key.starts_with("conversation:"))
            .unwrap();
        assert_eq!(group.last_message_preview.as_deref(), Some("last"));
        assert_eq!(group.unread_count, 2);
        assert_eq!(group.channels[0].message_count, 4);
        assert_eq!(
            group.reply_targets[0].last_inbound_at.as_deref(),
            Some("2026-08-01T00:00:04.000Z")
        );
        let customer = rows.iter().find(|row| row.key == "lead:7").unwrap();
        assert_eq!(customer.last_message_preview.as_deref(), Some("re"));
        assert_eq!(customer.unread_count, 0);
        assert_eq!(
            customer.reply_targets[0].last_inbound_at.as_deref(),
            Some("2026-08-01T00:00:05.000Z")
        );
    }

    #[test]
    fn last_message_lookup_has_an_index() {
        let harness = ServiceHarness::new();
        let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
        let plan: String = conn
            .query_row(
                "EXPLAIN QUERY PLAN SELECT text_content FROM messages WHERE conversation_id = 1
                 ORDER BY COALESCE(provider_timestamp, ingested_at) DESC, id DESC LIMIT 1",
                [],
                |row| row.get(3),
            )
            .unwrap();
        assert!(plan.contains("idx_messages_conv_occurred"), "{plan}");
        assert!(!plan.contains("TEMP B-TREE"), "{plan}");
    }

    #[test]
    #[ignore = "needs INT7_BENCH_DB, a copy of the v3-dev database"]
    fn n2_inbox_bench() {
        let Ok(path) = std::env::var("INT7_BENCH_DB") else {
            return;
        };
        assert!(
            path.starts_with("/tmp/"),
            "the bench runs on a copy under /tmp only"
        );
        // The app migrates at start-up, before any inbox call.
        let conn = crate::db::open_db_connection(&path).unwrap();
        crate::conversation::migration::migrate(&conn).unwrap();
        drop(conn);
        let repository = ConversationRepository::new(path);
        for run in 0..6 {
            let started = Instant::now();
            let items = repository.list_inbox().unwrap();
            let listed = started.elapsed();
            let page = page_inbox(items, &InboxQuery::default(), None, 50).unwrap();
            println!(
                "inboxPage run {run}: list_inbox {:.1} ms, total {:.1} ms, rows {}, first page {}",
                listed.as_secs_f64() * 1000.0,
                started.elapsed().as_secs_f64() * 1000.0,
                page.total,
                page.items.len()
            );
        }
    }
}
