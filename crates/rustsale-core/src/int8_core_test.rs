//! Lows found by the int7 QA that live in the core (#55). Each test names its item.

/// Deleting a row that is not there is `not found`, never a silent success.
mod delete_missing {
    use crate::conversation::error::ConversationErrorCode;
    use crate::conversation::migration::migrate;
    use crate::conversation::quick_templates::QuickTemplateRepository;
    use crate::conversation::types::CreateConversationQuickTemplateRequest;
    use crate::{campaign, commands, db};

    #[test]
    fn deleting_a_lead_twice_succeeds_once_then_is_not_found() {
        let mut conn = db::init_db(":memory:").unwrap();
        let id = commands::add_lead_internal(
            &mut conn,
            "Nguyen Van A".into(),
            "ACME".into(),
            "New".into(),
            0.0,
            String::new(),
            String::new(),
            String::new(),
            None,
            None,
        )
        .unwrap();
        commands::delete_lead_internal(&mut conn, id).unwrap();
        assert_eq!(
            commands::delete_lead_internal(&mut conn, id).unwrap_err(),
            format!("lead {id} not found")
        );
        assert_eq!(
            commands::delete_lead_internal(&mut conn, 777).unwrap_err(),
            "lead 777 not found"
        );
    }

    #[test]
    fn deleting_a_campaign_twice_succeeds_once_then_is_not_found() {
        let conn = db::init_db(":memory:").unwrap();
        let id = db::create_campaign(&conn, "Spring", None, "zalo", "DRAFT", None, None).unwrap();
        campaign::commands::delete_unified_campaign_internal(&conn, id).unwrap();
        assert_eq!(
            campaign::commands::delete_unified_campaign_internal(&conn, id).unwrap_err(),
            format!("campaign {id} not found")
        );
        assert_eq!(
            campaign::commands::delete_unified_campaign_internal(&conn, 777).unwrap_err(),
            "campaign 777 not found"
        );
    }

    #[test]
    fn a_missing_quick_template_is_not_found_on_delete_update_and_use() {
        let path = std::env::temp_dir().join(format!(
            "rustsale-int8-quick-template-{}.db",
            rand::random::<u64>()
        ));
        let conn = db::open_db_connection(path.to_str().unwrap()).unwrap();
        migrate(&conn).unwrap();
        drop(conn);
        let repo = QuickTemplateRepository::new(path.to_string_lossy().to_string());
        let created = repo
            .create(CreateConversationQuickTemplateRequest {
                shortcut: "/hi".into(),
                title: "Hello".into(),
                content: "Hello there".into(),
                category: None,
            })
            .unwrap();
        repo.delete(created.id).unwrap();
        for result in [repo.delete(created.id), repo.delete(777)] {
            let error = result.unwrap_err();
            assert_eq!(error.code, ConversationErrorCode::NotFound);
            assert_eq!(error.code.as_str(), "not_found");
        }
        assert_eq!(
            repo.mark_used(created.id).unwrap_err().code,
            ConversationErrorCode::NotFound
        );
        let _ = std::fs::remove_file(&path);
    }
}

/// A WhatsApp campaign with no account says what is missing, in words the API sorts into
/// `invalid_request`.
mod whatsapp_no_account {
    use crate::conversation::whatsapp::campaign::{create_campaign, CreateCampaignRequest};
    use crate::db;

    #[test]
    fn a_campaign_without_an_account_names_the_missing_account() {
        let conn = db::init_db(":memory:").unwrap();
        let request = CreateCampaignRequest {
            name: "No account".into(),
            template_id: None,
            assigned_account_ids: Vec::new(),
            rate_limit_min_delay_secs: None,
            rate_limit_max_delay_secs: None,
            burst_limit_per_batch: None,
            burst_pause_secs: None,
            quiet_hours_enabled: None,
            quiet_hours_start: None,
            quiet_hours_end: None,
            timezone: None,
        };
        let message = create_campaign(&conn, &request).unwrap_err().to_string();
        assert!(
            message.starts_with("Invalid campaign: at least one WhatsApp account is required"),
            "{message}"
        );
    }
}

/// AI proposals move through the review flow only along allowed transitions.
mod proposal_transitions {
    use crate::ai::review_ai_proposal_internal as review;
    use crate::db;

    fn proposal(conn: &rusqlite::Connection) -> i64 {
        db::create_ai_action_proposal(conn, None, None, "zalo", "text", "because").unwrap()
    }

    fn status(conn: &rusqlite::Connection, id: i64) -> String {
        db::get_ai_action_proposal(conn, id)
            .unwrap()
            .unwrap()
            .status
    }

    #[test]
    fn a_rejected_proposal_cannot_jump_to_approved() {
        let conn = db::init_db(":memory:").unwrap();
        let id = proposal(&conn);
        review(&conn, id, "REJECTED", "Reviewer").unwrap();
        for target in ["APPROVED", "APPLIED", "REJECTED"] {
            let error = review(&conn, id, target, "Reviewer").unwrap_err();
            assert!(error.contains("already REJECTED"), "{error}");
            assert_eq!(status(&conn, id), "REJECTED", "{target}");
        }
        // Reopened for review, it can then be approved.
        review(&conn, id, "PENDING", "Reviewer").unwrap();
        review(&conn, id, "APPROVED", "Reviewer").unwrap();
        assert_eq!(status(&conn, id), "APPROVED");
    }

    #[test]
    fn the_happy_path_and_its_undo_are_allowed() {
        let conn = db::init_db(":memory:").unwrap();
        let id = proposal(&conn);
        review(&conn, id, "approved", "Reviewer").unwrap();
        review(&conn, id, "PENDING", "Reviewer").unwrap();
        review(&conn, id, "APPROVED", "Reviewer").unwrap();
        review(&conn, id, "APPLIED", "Reviewer").unwrap();
        assert_eq!(status(&conn, id), "APPLIED");
    }

    #[test]
    fn an_applied_proposal_is_final() {
        let conn = db::init_db(":memory:").unwrap();
        let id = proposal(&conn);
        review(&conn, id, "APPROVED", "Reviewer").unwrap();
        review(&conn, id, "APPLIED", "Reviewer").unwrap();
        for target in ["PENDING", "APPROVED", "REJECTED", "APPLIED"] {
            let error = review(&conn, id, target, "Reviewer").unwrap_err();
            assert!(error.contains("already APPLIED"), "{error}");
        }
        assert_eq!(status(&conn, id), "APPLIED");
    }

    #[test]
    fn a_pending_proposal_is_not_applied_before_it_is_approved() {
        let conn = db::init_db(":memory:").unwrap();
        let id = proposal(&conn);
        let error = review(&conn, id, "APPLIED", "Reviewer").unwrap_err();
        assert!(error.contains("already PENDING"), "{error}");
        assert_eq!(status(&conn, id), "PENDING");
    }

    #[test]
    fn a_missing_proposal_and_a_bad_status_keep_their_errors() {
        let conn = db::init_db(":memory:").unwrap();
        assert_eq!(
            review(&conn, 4242, "APPROVED", "Reviewer").unwrap_err(),
            "AI proposal 4242 not found"
        );
        let id = proposal(&conn);
        assert!(review(&conn, id, "MAYBE", "Reviewer")
            .unwrap_err()
            .starts_with("Invalid status"));
    }
}
