//! Faults found by the UI2 QA that live in the core: unknown channels, edits of rows that are
//! gone, ids that point at nothing. Each test names the item of the ui2-core brief.

use super::commands;

#[test]
fn item2_unknown_template_channels_are_invalid() {
    for channel in ["bogus", "", "ZALO", " zalo", "sms"] {
        let error = commands::validate_template_channel(channel).unwrap_err();
        assert!(error.contains("invalid"), "{channel:?}: {error}");
    }
    for channel in commands::TEMPLATE_CHANNELS {
        assert_eq!(commands::validate_template_channel(channel), Ok(()));
    }
}

mod item3 {
    use crate::{commands, db, models};

    fn assert_not_found<T: std::fmt::Debug>(result: Result<T, String>, what: &str, id: i64) {
        let error = result.expect_err("write to a missing row must fail");
        assert_eq!(error, format!("{what} {id} not found"));
    }

    #[test]
    fn updates_on_missing_rows_are_not_found() {
        let conn = db::init_db(":memory:").unwrap();
        assert_not_found(
            commands::update_lead_status_internal(&conn, 777, "New".to_string()),
            "lead",
            777,
        );
        assert_not_found(
            commands::update_deal_stage_internal(&conn, 777, "Demo".to_string()),
            "deal",
            777,
        );
        assert_not_found(
            commands::update_lead_with_currency_internal(
                &conn,
                777,
                "A".into(),
                "B".into(),
                "New".into(),
                1.0,
                "o".into(),
                "e@x.io".into(),
                "1".into(),
                None,
                None,
                None,
            ),
            "lead",
            777,
        );
        assert_not_found(
            commands::update_email_template_internal(&conn, 777, "n", "s", "c"),
            "email template",
            777,
        );
        assert_not_found(
            commands::update_outreach_template_internal(&conn, 777, "n", "c", None),
            "outreach template",
            777,
        );
        let template = models::UpdateTemplateInput {
            id: 777,
            folder_id: None,
            name: "n".into(),
            channel: "zalo".into(),
            stage: None,
            subject: None,
            content: "c".into(),
            media_url: None,
            ai_prompt: None,
            tags_json: None,
        };
        assert_not_found(
            commands::update_template_internal(&conn, &template),
            "template",
            777,
        );
        let smtp = models::SmtpServer {
            id: Some(777),
            name: "n".into(),
            host: "h".into(),
            port: 587,
            username: "u".into(),
            password: "p".into(),
            from_email: "f@x.io".into(),
            from_name: "f".into(),
            security: "tls".into(),
            created_at: String::new(),
        };
        assert_not_found(
            commands::save_smtp_server_internal(&conn, &smtp),
            "SMTP server",
            777,
        );
    }

    #[test]
    fn updates_on_existing_rows_still_succeed() {
        let mut conn = db::init_db(":memory:").unwrap();
        db::seed_data(&mut conn).unwrap();
        commands::update_lead_status_internal(&conn, 1, "Contacted".to_string()).unwrap();
        commands::update_deal_stage_internal(&conn, 1, "Demo".to_string()).unwrap();
    }
}
