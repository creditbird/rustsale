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
