#[cfg(test)]
mod tests {
    use super::super::commands;
    use super::super::db;

    #[test]
    fn test_crm_commands() {
        let mut conn = db::init_db(":memory:").unwrap();
        db::seed_data(&mut conn).unwrap();

        // Test get_leads_internal
        let leads = commands::get_leads_internal(&conn).unwrap();
        assert_eq!(leads.len(), 3);
        assert_eq!(leads[0].name, "Sarah Harrington");
        assert_eq!(leads[0].about, Some("CTO of Acme Corp".to_string()));

        // Test get_lead_details
        let lead = commands::get_lead_details_internal(&conn, 1).unwrap();
        assert_eq!(lead.name, "Sarah Harrington");

        // Test add_lead_internal
        let new_lead_id = commands::add_lead_internal(
            &mut conn,
            "John Doe".to_string(),
            "Example Inc".to_string(),
            "New".to_string(),
            5000.0,
            "Alex Rivera".to_string(),
            "john@example.com".to_string(),
            "+11223344".to_string(),
            Some("CEO".to_string()),
            Some("123 Main St".to_string()),
        )
        .unwrap();
        assert!(new_lead_id > 0);

        let lead_retrieved = commands::get_lead_details_internal(&conn, new_lead_id).unwrap();
        assert_eq!(lead_retrieved.address, Some("123 Main St".to_string()));
        assert_eq!(lead_retrieved.has_zalo, None);
        assert_eq!(lead_retrieved.zalo_reached, 0);

        let leads_updated = commands::get_leads_internal(&conn).unwrap();
        assert_eq!(leads_updated.len(), 4);

        // Test update_lead_status_internal
        commands::update_lead_status_internal(&conn, new_lead_id, "Contacted".to_string()).unwrap();
        let lead_updated = commands::get_lead_details_internal(&conn, new_lead_id).unwrap();
        assert_eq!(lead_updated.status, "Contacted");

        // Test get_deals
        let deals = commands::get_deals_internal(&conn).unwrap();
        assert_eq!(deals.len(), 2);

        // Test update_deal_stage_internal
        commands::update_deal_stage_internal(&conn, 1, "Demo".to_string()).unwrap();
        let deals_updated = commands::get_deals_internal(&conn).unwrap();
        assert_eq!(deals_updated[0].stage, "Demo");

        // Test get_activities
        let activities = commands::get_activities_internal(&conn, 1).unwrap();
        assert_eq!(activities.len(), 1);
        assert_eq!(activities[0].activity_type, "Email");

        // Test add_activity_internal
        commands::add_activity_internal(
            &mut conn,
            1,
            "Call".to_string(),
            "Intro call".to_string(),
            "Alex Rivera".to_string(),
        )
        .unwrap();
        let activities_updated = commands::get_activities_internal(&conn, 1).unwrap();
        assert_eq!(activities_updated.len(), 2);
        assert_eq!(activities_updated[1].activity_type, "Call");
    }

    #[test]
    fn test_normalize_phone_digits_and_deduplication() {
        use crate::commands::normalize_phone_digits;
        assert_eq!(normalize_phone_digits("+84933001673"), "0933001673");
        assert_eq!(normalize_phone_digits("0933 001 673"), "0933001673");
        assert_eq!(normalize_phone_digits("0084933001673"), "0933001673");
        assert_eq!(normalize_phone_digits("0933-001-673"), "0933001673");
    }

    #[test]
    fn test_zalo_qr_and_session_commands() {
        let conn = db::init_db(":memory:").unwrap();

        // 1. Initial state check
        let sessions = commands::get_zalo_sessions_internal(&conn).unwrap();
        assert!(sessions.is_empty());

        // 2. Test QRCodeGenerated event translation
        let event_gen = zca_rs::apis::auth::LoginQREvent::QRCodeGenerated {
            code: "test-code".to_string(),
            image_base64: "base64-img".to_string(),
            token: "token-abc".to_string(),
        };
        let (evt, val) = commands::handle_zalo_qr_event(event_gen, None).unwrap();
        assert_eq!(evt, "zalo-qr-code");
        assert_eq!(val["code"], "test-code");
        assert_eq!(val["image_base64"], "base64-img");
        assert_eq!(val["token"], "token-abc");

        // 3. Test QRCodeScanned event translation
        let event_scan = zca_rs::apis::auth::LoginQREvent::QRCodeScanned {
            avatar: "avatar-url".to_string(),
            display_name: "John Doe".to_string(),
        };
        let (evt, val) = commands::handle_zalo_qr_event(event_scan, None).unwrap();
        assert_eq!(evt, "zalo-qr-scanned");
        assert_eq!(val["avatar"], "avatar-url");
        assert_eq!(val["display_name"], "John Doe");

        // 4. Test QRCodeExpired event translation
        let event_exp = zca_rs::apis::auth::LoginQREvent::QRCodeExpired;
        let (evt, val) = commands::handle_zalo_qr_event(event_exp, None).unwrap();
        assert_eq!(evt, "zalo-qr-expired");
        assert!(val.is_null());

        // 5. Test QRCodeDeclined event translation
        let event_dec = zca_rs::apis::auth::LoginQREvent::QRCodeDeclined;
        let (evt, val) = commands::handle_zalo_qr_event(event_dec, None).unwrap();
        assert_eq!(evt, "zalo-qr-declined");
        assert!(val.is_null());

        // 6. Test GotLoginInfo event translation and persistence
        let dummy_cookies = vec![zca_rs::context::ZaloCookie {
            name: "zpw_sek".to_string(),
            value: "sek-123456".to_string(),
            domain: "zalo.me".to_string(),
            path: "/".to_string(),
        }];
        let event_success = zca_rs::apis::auth::LoginQREvent::GotLoginInfo {
            cookie: dummy_cookies,
            imei: "imei-999".to_string(),
            user_agent: "Mozilla/MockUA".to_string(),
        };
        let (evt, val) = commands::handle_zalo_qr_event(event_success, Some(&conn)).unwrap();
        assert_eq!(evt, "zalo-qr-success");
        assert_eq!(val["imei"], "imei-999");

        // Verify session was persisted to database
        let saved_sessions = commands::get_zalo_sessions_internal(&conn).unwrap();
        assert_eq!(saved_sessions.len(), 1);
        assert_eq!(saved_sessions[0].imei, "imei-999");
        assert_eq!(saved_sessions[0].user_agent, "Mozilla/MockUA");
        assert_eq!(saved_sessions[0].language, "vi");

        let cookies: Vec<zca_rs::context::ZaloCookie> =
            serde_json::from_str(&saved_sessions[0].cookies_json).unwrap();
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0].name, "zpw_sek");
        assert_eq!(cookies[0].value, "sek-123456");

        // 7. Test delete session
        commands::delete_zalo_session_internal(&conn, "imei-999").unwrap();
        let sessions_after_delete = commands::get_zalo_sessions_internal(&conn).unwrap();
        assert!(sessions_after_delete.is_empty());
    }

    #[tokio::test]
    async fn test_zalo_session_status_serialization_and_verification_internal() {
        use crate::models::ZaloSession;
        use crate::models::ZaloSessionStatus;

        // Test serialization / deserialization
        let status = ZaloSessionStatus {
            imei: "imei-123".to_string(),
            is_valid: true,
            display_name: Some("Test User".to_string()),
            avatar: Some("https://avatar.url".to_string()),
            error_message: None,
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains(r#""imei":"imei-123""#));
        assert!(json.contains(r#""is_valid":true"#));
        assert!(json.contains(r#""display_name":"Test User""#));

        let deserialized: ZaloSessionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, deserialized);

        // Test invalid cookies JSON in verify_zalo_session_internal
        let invalid_session = ZaloSession {
            imei: "imei-bad-cookies".to_string(),
            user_agent: "MockUA".to_string(),
            cookies_json: "not valid json".to_string(),
            language: "vi".to_string(),
            avatar: None,
            display_name: None,
        };

        let verify_res = commands::verify_zalo_session_internal(invalid_session)
            .await
            .unwrap();
        assert_eq!(verify_res.imei, "imei-bad-cookies");
        assert!(!verify_res.is_valid);
        assert!(verify_res
            .error_message
            .unwrap()
            .contains("Invalid cookies format"));
    }

    #[tokio::test]
    async fn test_verify_zalo_session_commands_with_state() {
        use tauri::test::{mock_builder, mock_context, noop_assets};
        use tauri::Manager;

        let conn = db::init_db(":memory:").unwrap();
        let db_state = commands::DbState {
            conn: std::sync::Mutex::new(conn),
        };

        let app = mock_builder()
            .manage(db_state)
            .build(mock_context(noop_assets()))
            .unwrap();

        let state = app.state::<commands::DbState>();

        // Test verify_all_zalo_sessions on empty DB
        let all_statuses = commands::verify_all_zalo_sessions(state.clone())
            .await
            .unwrap();
        assert!(all_statuses.is_empty());

        // Test verify_zalo_session with non-existent IMEI
        let err = commands::verify_zalo_session("non-existent-imei".to_string(), state.clone())
            .await
            .unwrap_err();
        assert!(err.contains("Session with IMEI non-existent-imei not found"));
    }

    #[test]
    fn test_zalo_qr_session_rejects_callback_without_device_imei() {
        let conn = db::init_db(":memory:").unwrap();
        let user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64)".to_string();
        let event = zca_rs::apis::auth::LoginQREvent::GotLoginInfo {
            cookie: vec![zca_rs::context::ZaloCookie {
                name: "zpw_sek".to_string(),
                value: "sek-123456".to_string(),
                domain: "zalo.me".to_string(),
                path: "/".to_string(),
            }],
            imei: String::new(),
            user_agent,
        };

        let error = commands::handle_zalo_qr_event(event, Some(&conn)).unwrap_err();
        assert_eq!(error, "Zalo returned a session without a device IMEI");

        let sessions = commands::get_zalo_sessions_internal(&conn).unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_persist_authenticated_zalo_session() {
        let conn = db::init_db(":memory:").unwrap();
        let cookies = vec![zca_rs::context::ZaloCookie {
            name: "zpw_sek".to_string(),
            value: "sek-123456".to_string(),
            domain: "zalo.me".to_string(),
            path: "/".to_string(),
        }];

        let payload = commands::persist_authenticated_zalo_session(
            &conn,
            "imei-final",
            "Mozilla/FinalUA",
            &cookies,
        )
        .unwrap();

        assert_eq!(payload.imei, "imei-final");
        let sessions = commands::get_zalo_sessions_internal(&conn).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].imei, "imei-final");
        assert_eq!(sessions[0].user_agent, "Mozilla/FinalUA");
    }

    #[tokio::test]
    async fn test_send_zalo_outreach_mock() {
        use std::sync::Mutex;

        // Initialize a memory database
        let mut conn = db::init_db(":memory:").unwrap();
        db::seed_data(&mut conn).unwrap();

        // 1. Create a template
        let template_id =
            db::create_outreach_template(&conn, "Template Welcome", "Hello [name]!", None).unwrap();

        // 2. Add some leads
        // Lead 1: valid phone
        let lead_id_1 = commands::add_lead_internal(
            &mut conn,
            "Sarah Harrington".to_string(),
            "Acme Corp".to_string(),
            "Qualified".to_string(),
            45000.0,
            "Alex Rivera".to_string(),
            "sarah@acme.com".to_string(),
            "0912345678".to_string(),
            Some("CTO".to_string()),
            None,
        )
        .unwrap();

        // Lead 2: missing phone
        let lead_id_2 = commands::add_lead_internal(
            &mut conn,
            "No Phone Lead".to_string(),
            "No Phone Inc".to_string(),
            "New".to_string(),
            0.0,
            "Alex Rivera".to_string(),
            "nophone@example.com".to_string(),
            "".to_string(),
            None,
            None,
        )
        .unwrap();

        // Lead 3: phone that won't be found (mocking "not-found")
        let lead_id_3 = commands::add_lead_internal(
            &mut conn,
            "Not Found Lead".to_string(),
            "Not Found Inc".to_string(),
            "New".to_string(),
            0.0,
            "Alex Rivera".to_string(),
            "notfound@example.com".to_string(),
            "not-found".to_string(),
            None,
            None,
        )
        .unwrap();

        // Lead 4: landline phone (should be skipped)
        let lead_id_4 = commands::add_lead_internal(
            &mut conn,
            "Landline Lead".to_string(),
            "Landline Inc".to_string(),
            "New".to_string(),
            0.0,
            "Alex Rivera".to_string(),
            "landline@example.com".to_string(),
            "+12345678".to_string(),
            None,
            None,
        )
        .unwrap();

        let db_mutex = Mutex::new(conn);

        // Run send_zalo_outreach_internal
        let res = commands::send_zalo_outreach_internal(
            None,
            &db_mutex,
            "mock-imei".to_string(),
            vec![lead_id_1, lead_id_2, lead_id_3, lead_id_4],
            template_id,
            commands::ZaloOutreachRunOptions::default(),
            commands::OutreachProgressScope::default(),
        )
        .await;

        assert_eq!(res.unwrap().total_sent, 1);

        // Verify that activities log was written for lead_id_1 but not others
        let conn_locked = db_mutex.lock().unwrap();
        let activities_1 = commands::get_activities_internal(&conn_locked, lead_id_1).unwrap();
        assert_eq!(activities_1.len(), 1);
        assert_eq!(activities_1[0].activity_type, "Outreach");
        assert_eq!(
            activities_1[0].content,
            "Zalo message sent: Hello Sarah Harrington!"
        );
        assert_eq!(activities_1[0].performed_by, "System (Zalo Outreach)");

        let activities_2 = commands::get_activities_internal(&conn_locked, lead_id_2).unwrap();
        assert!(activities_2.is_empty());

        let activities_3 = commands::get_activities_internal(&conn_locked, lead_id_3).unwrap();
        assert!(activities_3.is_empty());

        let activities_4 = commands::get_activities_internal(&conn_locked, lead_id_4).unwrap();
        assert!(activities_4.is_empty());

        // Verify lead's last activity updated
        let lead_1 = commands::get_lead_details_internal(&conn_locked, lead_id_1).unwrap();
        assert_ne!(lead_1.last_activity, "");
        assert_eq!(lead_1.has_zalo, Some(1));
        assert_eq!(lead_1.zalo_reached, 1);

        let lead_3 = commands::get_lead_details_internal(&conn_locked, lead_id_3).unwrap();
        assert_eq!(lead_3.has_zalo, Some(0));
        assert_eq!(lead_3.zalo_reached, 0);

        let lead_4 = commands::get_lead_details_internal(&conn_locked, lead_id_4).unwrap();
        assert_eq!(lead_4.has_zalo, Some(0));

        // Verify outreach_history record exists for lead_id_1
        let history_count: i64 = conn_locked
            .query_row(
                "SELECT COUNT(*) FROM outreach_history WHERE lead_id = ?1 AND status = 'SUCCESS'",
                [lead_id_1],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(history_count, 1);

        // Verify conversation and message persistence
        let direct_conv_count: i64 = conn_locked
            .query_row(
                "SELECT COUNT(*) FROM channel_conversations WHERE kind = 'direct'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(direct_conv_count, 1);

        let group_conv_count: i64 = conn_locked
            .query_row(
                "SELECT COUNT(*) FROM channel_conversations WHERE kind = 'group'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(group_conv_count, 0);

        let total_messages: i64 = conn_locked
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
            .unwrap();
        assert_eq!(total_messages, 1);

        let (text_content, content_json): (String, String) = conn_locked
            .query_row(
                "SELECT text_content, content_json FROM messages LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(text_content.contains("Sarah Harrington"));
        assert!(content_json.contains("zalo_outreach"));
    }

    #[tokio::test]
    async fn test_send_zalo_outreach_burst_limit() {
        use std::sync::Mutex;

        let mut conn = db::init_db(":memory:").unwrap();
        db::seed_data(&mut conn).unwrap();

        let template_id =
            db::create_outreach_template(&conn, "Template Burst", "Hello [name]!", None).unwrap();

        let lead_id_1 = commands::add_lead_internal(
            &mut conn,
            "Lead One".to_string(),
            "Company A".to_string(),
            "New".to_string(),
            1000.0,
            "Alex".to_string(),
            "lead1@example.com".to_string(),
            "0912345678".to_string(),
            None,
            None,
        )
        .unwrap();

        let lead_id_2 = commands::add_lead_internal(
            &mut conn,
            "Lead Two".to_string(),
            "Company B".to_string(),
            "New".to_string(),
            1000.0,
            "Alex".to_string(),
            "lead2@example.com".to_string(),
            "0912345679".to_string(),
            None,
            None,
        )
        .unwrap();

        let lead_id_3 = commands::add_lead_internal(
            &mut conn,
            "Lead Three".to_string(),
            "Company C".to_string(),
            "New".to_string(),
            1000.0,
            "Alex".to_string(),
            "lead3@example.com".to_string(),
            "0912345680".to_string(),
            None,
            None,
        )
        .unwrap();

        let db_mutex = Mutex::new(conn);

        let res = commands::send_zalo_outreach_internal(
            None,
            &db_mutex,
            "mock-imei".to_string(),
            vec![lead_id_1, lead_id_2, lead_id_3],
            template_id,
            commands::ZaloOutreachRunOptions {
                enable_burst_limit: true,
                burst_max_messages: 2,
                burst_delay_seconds: 1,
                inter_message_delay_seconds: 0,
            },
            commands::OutreachProgressScope::default(),
        )
        .await;

        assert_eq!(res.unwrap().total_sent, 3);

        let conn_locked = db_mutex.lock().unwrap();
        let activities_1 = commands::get_activities_internal(&conn_locked, lead_id_1).unwrap();
        assert_eq!(activities_1.len(), 1);
        let activities_2 = commands::get_activities_internal(&conn_locked, lead_id_2).unwrap();
        assert_eq!(activities_2.len(), 1);
        let activities_3 = commands::get_activities_internal(&conn_locked, lead_id_3).unwrap();
        assert_eq!(activities_3.len(), 1);
    }

    #[test]
    fn test_category_commands() {
        use tauri::test::{mock_builder, mock_context, noop_assets};
        use tauri::Manager;

        let conn = db::init_db(":memory:").unwrap();
        let mut conn = conn;
        db::seed_data(&mut conn).unwrap();

        let db_state = commands::DbState {
            conn: std::sync::Mutex::new(conn),
        };

        let app = mock_builder()
            .manage(db_state)
            .build(mock_context(noop_assets()))
            .unwrap();

        let state = app.state::<commands::DbState>();

        // 1. Initially categories list should be empty
        let initial_categories = commands::get_categories(state.clone()).unwrap();
        assert!(initial_categories.is_empty());

        // 2. Failure path: empty or whitespace category name should return an error
        let res_empty = commands::create_category(state.clone(), "".to_string());
        assert!(res_empty.is_err());
        assert_eq!(res_empty.err().unwrap(), "Category name cannot be empty");

        let res_whitespace = commands::create_category(state.clone(), "   ".to_string());
        assert!(res_whitespace.is_err());
        assert_eq!(
            res_whitespace.err().unwrap(),
            "Category name cannot be empty"
        );

        // 3. Create a valid category
        let category_id = commands::create_category(state.clone(), "Partners".to_string()).unwrap();
        assert!(category_id > 0);

        // 4. Verify get_categories now returns the category
        let categories2 = commands::get_categories(state.clone()).unwrap();
        assert_eq!(categories2.len(), 1);
        assert_eq!(categories2[0].name, "Partners");
        assert_eq!(categories2[0].id, Some(category_id));

        // 5. Failure path: adding a non-existent lead ID (e.g. 99999) should return an error
        let res_nonexistent =
            commands::add_leads_to_category(state.clone(), vec![99999], category_id);
        assert!(res_nonexistent.is_err());

        // 6. Add valid leads to category (leads 1 and 2 exist in seed data)
        let res_add = commands::add_leads_to_category(state.clone(), vec![1, 2], category_id);
        assert!(res_add.is_ok());

        // 7. Verify get_leads_by_category returns 2 leads
        let category_leads = commands::get_leads_by_category(state.clone(), category_id).unwrap();
        assert_eq!(category_leads.len(), 2);
        assert!(category_leads.iter().any(|l| l.name == "Sarah Harrington"));
        assert!(category_leads.iter().any(|l| l.name == "Jordan Smith"));

        // 8. Remove lead 1 from category
        let res_remove = commands::remove_leads_from_category(state.clone(), vec![1], category_id);
        assert!(res_remove.is_ok());

        // Verify get_leads_by_category now returns 1 lead (Jordan Smith)
        let category_leads2 = commands::get_leads_by_category(state.clone(), category_id).unwrap();
        assert_eq!(category_leads2.len(), 1);
        assert_eq!(category_leads2[0].name, "Jordan Smith");

        // 9. Delete category
        let res_delete = commands::delete_category(state.clone(), category_id);
        assert!(res_delete.is_ok());

        // Verify get_categories is empty again
        let final_categories = commands::get_categories(state.clone()).unwrap();
        assert!(final_categories.is_empty());

        // Verify that deleting a category cleans up its join table entries (cascade delete worked)
        // Query the join table directly
        let join_count: i32 = {
            let conn_guard = state.conn.lock().unwrap();
            conn_guard
                .query_row(
                    "SELECT COUNT(*) FROM lead_categories WHERE category_id = ?1",
                    [category_id],
                    |row| row.get(0),
                )
                .unwrap()
        };
        assert_eq!(join_count, 0);

        // Verify get_leads_by_category shows no association exists
        let category_leads3 = commands::get_leads_by_category(state.clone(), category_id).unwrap();
        assert!(category_leads3.is_empty());
    }

    #[tokio::test]
    async fn test_send_zalo_campaign_mock() {
        use std::sync::Mutex;
        let mut conn = db::init_db(":memory:").unwrap();
        db::seed_data(&mut conn).unwrap();

        let template_id =
            db::create_outreach_template(&conn, "Campaign Template", "Hello {name}!", None)
                .unwrap();

        let category_id = db::create_category(&conn, "Campaign Category").unwrap();

        let lead_1 = commands::add_lead_internal(
            &mut conn,
            "Valid Lead".to_string(),
            "Inc".to_string(),
            "New".to_string(),
            0.0,
            "Owner".to_string(),
            "".to_string(),
            "0912345678".to_string(),
            None,
            None,
        )
        .unwrap();

        let lead_2 = commands::add_lead_internal(
            &mut conn,
            "Missing Phone Lead".to_string(),
            "Inc".to_string(),
            "New".to_string(),
            0.0,
            "Owner".to_string(),
            "".to_string(),
            "".to_string(),
            None,
            None,
        )
        .unwrap();

        let lead_3 = commands::add_lead_internal(
            &mut conn,
            "Not Found Zalo Lead".to_string(),
            "Inc".to_string(),
            "New".to_string(),
            0.0,
            "Owner".to_string(),
            "".to_string(),
            "not-found".to_string(),
            None,
            None,
        )
        .unwrap();

        let lead_4 = commands::add_lead_internal(
            &mut conn,
            "Mock Cached Lead".to_string(),
            "Inc".to_string(),
            "New".to_string(),
            0.0,
            "Owner".to_string(),
            "".to_string(),
            "0987654321".to_string(),
            None,
            None,
        )
        .unwrap();

        // Add all to category
        db::add_lead_to_category(&conn, lead_1, category_id).unwrap();
        db::add_lead_to_category(&conn, lead_2, category_id).unwrap();
        db::add_lead_to_category(&conn, lead_3, category_id).unwrap();
        db::add_lead_to_category(&conn, lead_4, category_id).unwrap();

        // Manually set cached uid for lead 4
        conn.execute(
            "UPDATE leads SET zalo_uid = 'cached-123' WHERE id = ?1",
            [lead_4],
        )
        .unwrap();

        let db_mutex = Mutex::new(conn);

        let session_imeis = vec![
            "mock-fail-imei".to_string(),
            "mock-success-imei".to_string(),
        ];

        let res = commands::send_zalo_campaign_internal(
            None,
            &db_mutex,
            category_id,
            template_id,
            session_imeis,
        )
        .await;

        assert!(res.is_ok());

        let conn_locked = db_mutex.lock().unwrap();

        // Lead 1: valid phone, not cached.
        // 1st iteration uses active_index 0 ("mock-fail-imei") -> send fails, session rotated to 1.
        let l1 = commands::get_lead_details_internal(&conn_locked, lead_1).unwrap();
        assert_eq!(l1.zalo_reached, 0);
        let h1: i64 = conn_locked
            .query_row(
                "SELECT COUNT(*) FROM outreach_history WHERE lead_id = ?1 AND status = 'FAILED'",
                [lead_1],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(h1, 1);

        // Lead 2: missing phone, skipped early.
        let l2 = commands::get_lead_details_internal(&conn_locked, lead_2).unwrap();
        assert_eq!(l2.zalo_reached, 0);

        // Lead 3: not found.
        // 2nd iteration uses active_index 1 ("mock-success-imei")
        // But lookup fails.
        let l3 = commands::get_lead_details_internal(&conn_locked, lead_3).unwrap();
        assert_eq!(l3.has_zalo, Some(0));
        assert_eq!(l3.zalo_reached, 0);

        // Lead 4: cached uid.
        // 3rd iteration uses active_index 1 ("mock-success-imei") -> success.
        let l4 = commands::get_lead_details_internal(&conn_locked, lead_4).unwrap();
        assert_eq!(l4.zalo_reached, 1);
        let h4: i64 = conn_locked
            .query_row(
                "SELECT COUNT(*) FROM outreach_history WHERE lead_id = ?1 AND status = 'SUCCESS'",
                [lead_4],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(h4, 1);
    }

    #[tokio::test]
    async fn test_send_zalo_campaign_continues_after_lead_send_failure() {
        use std::sync::Mutex;

        let mut conn = db::init_db(":memory:").unwrap();
        db::seed_data(&mut conn).unwrap();
        let template_id =
            db::create_outreach_template(&conn, "Campaign Template", "Hello [name]!", None)
                .unwrap();
        let category_id = db::create_category(&conn, "Campaign Category").unwrap();

        let lead_1 = commands::add_lead_internal(
            &mut conn,
            "First Lead".to_string(),
            "Inc".to_string(),
            "New".to_string(),
            0.0,
            "Owner".to_string(),
            "".to_string(),
            "0912345678".to_string(),
            None,
            None,
        )
        .unwrap();
        let lead_2 = commands::add_lead_internal(
            &mut conn,
            "Failing Lead".to_string(),
            "Inc".to_string(),
            "New".to_string(),
            0.0,
            "Owner".to_string(),
            "".to_string(),
            "mock-send-fail".to_string(),
            None,
            None,
        )
        .unwrap();
        let lead_3 = commands::add_lead_internal(
            &mut conn,
            "Last Lead".to_string(),
            "Inc".to_string(),
            "New".to_string(),
            0.0,
            "Owner".to_string(),
            "".to_string(),
            "0987654321".to_string(),
            None,
            None,
        )
        .unwrap();

        for lead_id in [lead_1, lead_2, lead_3] {
            db::add_lead_to_category(&conn, lead_id, category_id).unwrap();
            conn.execute(
                "UPDATE leads SET zalo_uid = ?1 WHERE id = ?2",
                rusqlite::params![format!("uid-{lead_id}"), lead_id],
            )
            .unwrap();
        }

        let db_mutex = Mutex::new(conn);
        commands::send_zalo_campaign_internal(
            None,
            &db_mutex,
            category_id,
            template_id,
            vec!["mock-success-imei".to_string()],
        )
        .await
        .unwrap();

        let conn = db_mutex.lock().unwrap();
        assert_eq!(
            commands::get_lead_details_internal(&conn, lead_1)
                .unwrap()
                .zalo_reached,
            1
        );
        assert_eq!(
            commands::get_lead_details_internal(&conn, lead_2)
                .unwrap()
                .zalo_reached,
            0
        );
        assert_eq!(
            commands::get_lead_details_internal(&conn, lead_3)
                .unwrap()
                .zalo_reached,
            1
        );

        let failure: (i64, String) = conn
            .query_row(
                "SELECT COUNT(*), COALESCE(MAX(error_message), '') FROM outreach_history WHERE lead_id = ?1 AND status = 'FAILED'",
                [lead_2],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(failure.0, 1);
        assert!(failure.1.contains("Mock lead send failure"));
    }

    #[tokio::test]
    async fn test_send_zalo_campaign_eviction_on_lookup_failure() {
        use std::sync::Mutex;
        let mut conn = db::init_db(":memory:").unwrap();
        db::seed_data(&mut conn).unwrap();

        let template_id =
            db::create_outreach_template(&conn, "Campaign Template", "Hello {name}!", None)
                .unwrap();
        let category_id = db::create_category(&conn, "Campaign Category").unwrap();

        let lead_1 = commands::add_lead_internal(
            &mut conn,
            "Rate Limited Lead".to_string(),
            "Inc".to_string(),
            "New".to_string(),
            0.0,
            "Owner".to_string(),
            "".to_string(),
            "mock-rate-limit".to_string(),
            None,
            None,
        )
        .unwrap();

        let lead_2 = commands::add_lead_internal(
            &mut conn,
            "Success Lead".to_string(),
            "Inc".to_string(),
            "New".to_string(),
            0.0,
            "Owner".to_string(),
            "".to_string(),
            "0912345678".to_string(),
            None,
            None,
        )
        .unwrap();

        db::add_lead_to_category(&conn, lead_1, category_id).unwrap();
        db::add_lead_to_category(&conn, lead_2, category_id).unwrap();

        // Lead 2 has cached uid to bypass lookup
        conn.execute(
            "UPDATE leads SET zalo_uid = 'cached-123' WHERE id = ?1",
            [lead_2],
        )
        .unwrap();

        let db_mutex = Mutex::new(conn);

        // mock-fail-imei is index 0, mock-success-imei is index 1
        let session_imeis = vec![
            "mock-fail-imei".to_string(),
            "mock-success-imei".to_string(),
        ];

        let res = commands::send_zalo_campaign_internal(
            None,
            &db_mutex,
            category_id,
            template_id,
            session_imeis,
        )
        .await;

        assert!(res.is_ok());

        let conn_locked = db_mutex.lock().unwrap();

        // Lead 1 lookup fails, so it is rate limited and session mock-fail-imei is evicted
        let l1 = commands::get_lead_details_internal(&conn_locked, lead_1).unwrap();
        assert_eq!(l1.zalo_status, Some("RATE_LIMITED".to_string()));

        // Lead 2 must have succeeded using mock-success-imei because mock-fail-imei was evicted.
        // If mock-fail-imei was NOT evicted, it would be used and fail.
        let l2 = commands::get_lead_details_internal(&conn_locked, lead_2).unwrap();
        assert_eq!(l2.zalo_reached, 1);
    }

    #[tokio::test]
    async fn test_send_zalo_campaign_exhausted_sessions() {
        use std::sync::Mutex;
        let mut conn = db::init_db(":memory:").unwrap();
        db::seed_data(&mut conn).unwrap();

        let template_id =
            db::create_outreach_template(&conn, "Campaign Template", "Hello {name}!", None)
                .unwrap();
        let category_id = db::create_category(&conn, "Campaign Category").unwrap();

        let lead_1 = commands::add_lead_internal(
            &mut conn,
            "Rate Limited Lead".to_string(),
            "Inc".to_string(),
            "New".to_string(),
            0.0,
            "Owner".to_string(),
            "".to_string(),
            "mock-rate-limit".to_string(),
            None,
            None,
        )
        .unwrap();

        let lead_2 = commands::add_lead_internal(
            &mut conn,
            "Unreached Lead".to_string(),
            "Inc".to_string(),
            "New".to_string(),
            0.0,
            "Owner".to_string(),
            "".to_string(),
            "0912345678".to_string(),
            None,
            None,
        )
        .unwrap();

        db::add_lead_to_category(&conn, lead_1, category_id).unwrap();
        db::add_lead_to_category(&conn, lead_2, category_id).unwrap();

        let db_mutex = Mutex::new(conn);

        // Only one failing session
        let session_imeis = vec!["mock-fail-imei".to_string()];

        let res = commands::send_zalo_campaign_internal(
            None,
            &db_mutex,
            category_id,
            template_id,
            session_imeis,
        )
        .await;

        assert!(res.is_ok());

        let conn_locked = db_mutex.lock().unwrap();

        let l2 = commands::get_lead_details_internal(&conn_locked, lead_2).unwrap();
        assert_eq!(l2.zalo_reached, 0);

        let h2_count: i64 = conn_locked
            .query_row(
                "SELECT COUNT(*) FROM outreach_history WHERE lead_id = ?1",
                [lead_2],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(h2_count, 0);
    }

    #[tokio::test]
    async fn test_smtp_and_email_template_commands() {
        use crate::models::SmtpServer;
        use tauri::test::{mock_builder, mock_context, noop_assets};
        use tauri::Manager;
        use tokio::net::TcpListener;

        let conn = db::init_db(":memory:").unwrap();
        let db_state = commands::DbState {
            conn: std::sync::Mutex::new(conn),
        };

        let app = mock_builder()
            .manage(db_state)
            .build(mock_context(noop_assets()))
            .unwrap();

        let state = app.state::<commands::DbState>();

        // 1. Initially get_smtp_servers should be empty
        let initial_servers = commands::get_smtp_servers(state.clone()).unwrap();
        assert!(initial_servers.is_empty());

        // 2. Save a new SMTP server
        let new_server = SmtpServer {
            id: None,
            name: "Gmail SMTP".to_string(),
            host: "smtp.gmail.com".to_string(),
            port: 587,
            username: "test@gmail.com".to_string(),
            password: "password123".to_string(),
            from_email: "test@gmail.com".to_string(),
            from_name: "Test Sender".to_string(),
            security: "STARTTLS".to_string(),
            created_at: "".to_string(),
        };

        let server_id = commands::save_smtp_server(new_server, state.clone()).unwrap();
        assert!(server_id > 0);

        // 3. Verify get_smtp_servers returns the saved server
        let servers = commands::get_smtp_servers(state.clone()).unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].id, Some(server_id));
        assert_eq!(servers[0].name, "Gmail SMTP");
        assert_eq!(servers[0].host, "smtp.gmail.com");

        // 4. Update the SMTP server
        let mut updated_server = servers[0].clone();
        updated_server.name = "Gmail Updated".to_string();
        updated_server.port = 465;

        let updated_id = commands::save_smtp_server(updated_server, state.clone()).unwrap();
        assert_eq!(updated_id, server_id);

        let servers_after_update = commands::get_smtp_servers(state.clone()).unwrap();
        assert_eq!(servers_after_update.len(), 1);
        assert_eq!(servers_after_update[0].name, "Gmail Updated");
        assert_eq!(servers_after_update[0].port, 465);

        // 5. Test SMTP connection to a mock TCP listener
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port() as i32;

        let test_res = commands::test_smtp_connection("127.0.0.1".to_string(), port).await;
        assert!(
            test_res.is_ok(),
            "Expected connection success to local listener: {:?}",
            test_res
        );

        // Test SMTP connection to unreachable port
        let fail_res = commands::test_smtp_connection("127.0.0.1".to_string(), 1).await;
        assert!(fail_res.is_err(), "Expected connection failure to port 1");

        // 6. Delete SMTP server
        let del_res = commands::delete_smtp_server(server_id, state.clone());
        assert!(del_res.is_ok());

        let servers_final = commands::get_smtp_servers(state.clone()).unwrap();
        assert!(servers_final.is_empty());

        // 7. Initially get_email_templates should be empty
        let initial_templates = commands::get_email_templates(state.clone()).unwrap();
        assert!(initial_templates.is_empty());

        // 8. Create Email Template
        let tpl_id = commands::create_email_template(
            "Welcome Email".to_string(),
            "Welcome {{name}}".to_string(),
            "Hello {{name}}, welcome to our platform!".to_string(),
            state.clone(),
        )
        .unwrap();
        assert!(tpl_id > 0);

        // 9. Verify get_email_templates
        let templates = commands::get_email_templates(state.clone()).unwrap();
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].id, Some(tpl_id));
        assert_eq!(templates[0].name, "Welcome Email");
        assert_eq!(templates[0].subject, "Welcome {{name}}");

        // 10. Update Email Template
        let update_tpl_res = commands::update_email_template(
            tpl_id,
            "Updated Welcome Email".to_string(),
            "Welcome aboard {{name}}".to_string(),
            "New content".to_string(),
            state.clone(),
        );
        assert!(update_tpl_res.is_ok());

        let templates_updated = commands::get_email_templates(state.clone()).unwrap();
        assert_eq!(templates_updated.len(), 1);
        assert_eq!(templates_updated[0].name, "Updated Welcome Email");
        assert_eq!(templates_updated[0].subject, "Welcome aboard {{name}}");
        assert_eq!(templates_updated[0].content, "New content");

        // 11. Delete Email Template
        let del_tpl_res = commands::delete_email_template(tpl_id, state.clone());
        assert!(del_tpl_res.is_ok());

        let templates_final = commands::get_email_templates(state.clone()).unwrap();
        assert!(templates_final.is_empty());
    }

    #[tokio::test]
    async fn test_email_outreach_commands() {
        use crate::models::{
            EmailOutreachCompletePayload, EmailOutreachLogPayload, EmailOutreachProgressPayload,
            Lead, SmtpServer,
        };

        // 1. Test replace_email_template_variables
        let lead = Lead {
            id: Some(1),
            name: "Jane Smith".to_string(),
            company: "Acme Corp".to_string(),
            status: "New".to_string(),
            value: 1000.0,
            last_activity: "2026-07-24".to_string(),
            owner: "Alice".to_string(),
            email: "jane@acme.com".to_string(),
            phone: "+15551234".to_string(),
            about: None,
            address: None,
            has_zalo: None,
            zalo_reached: 0,
            zalo_uid: None,
            zalo_status: None,
            relationship_status: None,
            last_lookup_attempt: None,
        };
        let tpl = "Hello {{name}} from {{company}}! Email: {{email}}, Phone: {{phone}}";
        let replaced = commands::replace_email_template_variables(tpl, &lead);
        assert_eq!(
            replaced,
            "Hello Jane Smith from Acme Corp! Email: jane@acme.com, Phone: +15551234"
        );

        // 2. Test payload serialization
        let progress_payload = EmailOutreachProgressPayload {
            progress: 50.0,
            status: "running".to_string(),
            message: "Processed 1/2 leads".to_string(),
        };
        let progress_json = serde_json::to_string(&progress_payload).unwrap();
        assert!(progress_json.contains("\"progress\":50.0"));
        assert!(progress_json.contains("\"status\":\"running\""));

        let log_payload = EmailOutreachLogPayload {
            message: "Sent email to jane@acme.com".to_string(),
        };
        let log_json = serde_json::to_string(&log_payload).unwrap();
        assert!(log_json.contains("jane@acme.com"));

        let complete_payload = EmailOutreachCompletePayload {
            total_sent: 2,
            message: "Email outreach completed. 2 emails sent.".to_string(),
        };
        let complete_json = serde_json::to_string(&complete_payload).unwrap();
        assert!(complete_json.contains("\"total_sent\":2"));

        // 3. Test send_email_outreach_internal with DB
        let mut conn = db::init_db(":memory:").unwrap();
        db::seed_data(&mut conn).unwrap();

        let smtp = SmtpServer {
            id: None,
            name: "Test SMTP".to_string(),
            host: "127.0.0.1".to_string(),
            port: 1, // unreachable port to fail fast
            username: "test@example.com".to_string(),
            password: "password".to_string(),
            from_email: "sender@example.com".to_string(),
            from_name: "Test Sender".to_string(),
            security: "STARTTLS".to_string(),
            created_at: "".to_string(),
        };
        let smtp_id = db::save_smtp_server(&conn, &smtp).unwrap();
        let tpl_id = db::create_email_template(
            &conn,
            "Welcome",
            "Hello {{name}}",
            "Welcome {{name}} to {{company}}",
        )
        .unwrap();

        let conn_mutex = std::sync::Mutex::new(conn);
        let res =
            commands::send_email_outreach_internal(None, &conn_mutex, smtp_id, tpl_id, vec![1, 2])
                .await;

        assert!(res.is_ok());

        // Check that history was logged for both leads
        let conn_guard = conn_mutex.lock().unwrap();
        let mut stmt = conn_guard
            .prepare("SELECT lead_id, status, error_message FROM email_outreach_history")
            .unwrap();
        let history_rows: Vec<(i32, String, Option<String>)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(history_rows.len(), 2);
        assert_eq!(history_rows[0].0, 1);
        assert_eq!(history_rows[0].1, "FAILED");
        assert!(history_rows[0].2.is_some());
    }

    #[tokio::test]
    async fn test_get_campaign_leads_never_sent_filtering() {
        let mut conn = crate::db::init_db(":memory:").unwrap();
        let cat_id = crate::db::create_category(&conn, "Test Cat").unwrap();

        let lead_1 = commands::add_lead_internal(
            &mut conn,
            "Unreached Lead".into(),
            "Inc".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "l1@test.com".into(),
            "0912345678".into(),
            None,
            None,
        )
        .unwrap();
        let lead_2 = commands::add_lead_internal(
            &mut conn,
            "Zalo Reached Flag Lead".into(),
            "Inc".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "l2@test.com".into(),
            "0987654321".into(),
            None,
            None,
        )
        .unwrap();
        let lead_3 = commands::add_lead_internal(
            &mut conn,
            "Outreach History Success Lead".into(),
            "Inc".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "l3@test.com".into(),
            "0933333333".into(),
            None,
            None,
        )
        .unwrap();

        for lead_id in &[lead_1, lead_2, lead_3] {
            crate::db::add_lead_to_category(&conn, *lead_id, cat_id).unwrap();
        }

        // Set zalo_reached = 1 for lead_2
        conn.execute("UPDATE leads SET zalo_reached = 1 WHERE id = ?1", [lead_2])
            .unwrap();

        // Create template
        let tpl_id = crate::db::create_outreach_template(&conn, "Test Tpl", "Hello", None).unwrap();

        // Insert outreach_history success for lead_3
        conn.execute(
            "INSERT INTO outreach_history (lead_id, template_id, status, sent_at, session_imei) VALUES (?1, ?2, 'SUCCESS', '2026-01-01T00:00:00Z', 'mock-imei')",
            rusqlite::params![lead_3, tpl_id],
        ).unwrap();

        let query_str = "SELECT l.id FROM leads l JOIN lead_categories lc ON l.id = lc.lead_id WHERE lc.category_id = ?1 AND COALESCE(l.zalo_reached, 0) = 0 AND l.id NOT IN (SELECT DISTINCT lead_id FROM outreach_history WHERE status = 'SUCCESS')";
        let mut stmt = conn.prepare(query_str).unwrap();
        let leads: Vec<i32> = stmt
            .query_map([cat_id], |row| row.get::<_, i32>(0))
            .unwrap()
            .collect::<Result<Vec<i32>, _>>()
            .unwrap();

        assert_eq!(leads.len(), 1);
        assert_eq!(leads[0], lead_1);
    }

    fn default_outreach_query() -> commands::OutreachLeadQuery {
        commands::OutreachLeadQuery {
            category_id: None,
            segmentation: "all".to_string(),
            filter_template_id: None,
            channel: "zalo".to_string(),
            search: None,
            filter_verified_zalo: false,
            filter_unreached: false,
            filter_mobile_only: false,
        }
    }

    #[test]
    fn test_outreach_lead_page_uses_cursor_filters_and_resolves_snapshot() {
        let mut conn = crate::db::init_db(":memory:").unwrap();
        let mut lead_ids = Vec::new();

        for index in 1..=75 {
            let lead_id = commands::add_lead_internal(
                &mut conn,
                format!("Lead {:03}", index),
                format!("Company {:03}", index),
                "New".into(),
                0.0,
                "Owner".into(),
                format!("lead{index}@example.com"),
                format!("09{index:08}"),
                None,
                None,
            )
            .unwrap();
            lead_ids.push(lead_id);
        }

        conn.execute(
            "UPDATE leads SET has_zalo = 1 WHERE id IN (?1, ?2)",
            rusqlite::params![lead_ids[10], lead_ids[20]],
        )
        .unwrap();

        let query = default_outreach_query();
        let first_page =
            commands::get_outreach_leads_page_internal(&conn, &query, None, 50).unwrap();

        assert_eq!(first_page.total, 75);
        assert_eq!(first_page.items.len(), 50);
        assert_eq!(first_page.items[0].id, Some(*lead_ids.last().unwrap()));
        assert_eq!(first_page.next_cursor, Some(lead_ids[25]));

        let second_page =
            commands::get_outreach_leads_page_internal(&conn, &query, first_page.next_cursor, 50)
                .unwrap();
        assert_eq!(second_page.total, 75);
        assert_eq!(second_page.items.len(), 25);
        assert_eq!(second_page.items[0].id, Some(lead_ids[24]));
        assert_eq!(second_page.next_cursor, None);

        let mut search_query = default_outreach_query();
        search_query.search = Some("Lead 007".to_string());
        let searched =
            commands::get_outreach_leads_page_internal(&conn, &search_query, None, 50).unwrap();
        assert_eq!(searched.total, 1);
        assert_eq!(searched.items[0].name, "Lead 007");

        let mut verified_query = default_outreach_query();
        verified_query.filter_verified_zalo = true;
        let verified =
            commands::get_outreach_leads_page_internal(&conn, &verified_query, None, 50).unwrap();
        assert_eq!(verified.total, 2);

        let excluded = vec![lead_ids[74], lead_ids[73]];
        let resolved =
            commands::resolve_outreach_lead_ids_internal(&conn, &query, &excluded).unwrap();
        assert_eq!(resolved.len(), 73);
        assert!(!resolved.contains(&lead_ids[74]));
        assert!(!resolved.contains(&lead_ids[73]));
        assert_eq!(resolved[0], lead_ids[72]);
    }

    #[test]
    fn selected_sessions_partition_leads_without_duplication() {
        let imeis = vec![
            "imei-a".to_string(),
            "imei-b".to_string(),
            "imei-a".to_string(),
        ];
        let lead_ids = vec![1, 2, 3, 4, 5, 2, 1];
        let assignments = commands::partition_outreach_leads(&imeis, &lead_ids).unwrap();

        assert_eq!(assignments.len(), 2);
        assert_eq!(assignments[0], ("imei-a".to_string(), vec![1, 3, 5]));
        assert_eq!(assignments[1], ("imei-b".to_string(), vec![2, 4]));

        let all = assignments
            .into_iter()
            .flat_map(|(_, ids)| ids)
            .collect::<Vec<_>>();
        assert_eq!(all.len(), 5);
        assert_eq!(
            all.iter().collect::<std::collections::HashSet<_>>().len(),
            5
        );

        let empty_imeis: Vec<String> = vec![];
        assert!(commands::partition_outreach_leads(&empty_imeis, &[1, 2]).is_err());
    }

    #[tokio::test]
    async fn test_start_zalo_outreach_multi_session_partition() {
        use std::sync::Mutex;

        let mut conn = db::init_db(":memory:").unwrap();
        db::seed_data(&mut conn).unwrap();

        let template_id =
            db::create_outreach_template(&conn, "Template Multi", "Hello [name]!", None).unwrap();

        let mut lead_ids = Vec::new();
        for i in 1..=5 {
            let id = commands::add_lead_internal(
                &mut conn,
                format!("Lead {}", i),
                format!("Company {}", i),
                "New".to_string(),
                1000.0,
                "Alex".to_string(),
                format!("lead{}@example.com", i),
                format!("091234560{}", i),
                None,
                None,
            )
            .unwrap();
            lead_ids.push(id);
        }

        let imeis = vec!["mock-imei-1".to_string(), "mock-imei-2".to_string()];
        let partitions = commands::partition_outreach_leads(&imeis, &lead_ids).unwrap();
        assert_eq!(partitions.len(), 2);
        assert_eq!(partitions[0].1, vec![lead_ids[0], lead_ids[2], lead_ids[4]]);
        assert_eq!(partitions[1].1, vec![lead_ids[1], lead_ids[3]]);

        let db_mutex = Mutex::new(conn);
        let total_leads = lead_ids.len();

        let res1 = commands::send_zalo_outreach_internal(
            None,
            &db_mutex,
            partitions[0].0.clone(),
            partitions[0].1.clone(),
            template_id,
            commands::ZaloOutreachRunOptions::default(),
            commands::OutreachProgressScope {
                processed_offset: 0,
                sent_offset: 0,
                total_leads,
                emit_complete: false,
            },
        )
        .await
        .unwrap();

        let res2 = commands::send_zalo_outreach_internal(
            None,
            &db_mutex,
            partitions[1].0.clone(),
            partitions[1].1.clone(),
            template_id,
            commands::ZaloOutreachRunOptions::default(),
            commands::OutreachProgressScope {
                processed_offset: partitions[0].1.len(),
                sent_offset: res1.total_sent,
                total_leads,
                emit_complete: true,
            },
        )
        .await
        .unwrap();

        assert_eq!(res1.total_sent + res2.total_sent, 5);

        let conn_locked = db_mutex.lock().unwrap();
        let total_history: i64 = conn_locked
            .query_row(
                "SELECT COUNT(*) FROM outreach_history WHERE status = 'SUCCESS'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(total_history, 5);
    }
}
