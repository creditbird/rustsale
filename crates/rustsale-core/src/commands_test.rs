#[cfg(test)]
mod tests {
    use super::super::commands;
    use super::super::db;
    use crate::host::AppHandle;

    const NO_APP_HANDLE: Option<&AppHandle> = None;

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
        assert_eq!(deals[0].currency, Some("VND".to_string()));

        // Test create_deal_internal with USD
        let usd_deal = commands::create_deal_internal(
            &conn,
            new_lead_id,
            "International Deal".to_string(),
            5000.0,
            "Prospecting".to_string(),
            Some("USD".to_string()),
        )
        .unwrap();
        assert_eq!(usd_deal.currency, Some("USD".to_string()));

        // Test create_deal_internal defaulting to VND
        let vnd_deal = commands::create_deal_internal(
            &conn,
            new_lead_id,
            "Local Deal".to_string(),
            10000000.0,
            "Prospecting".to_string(),
            None,
        )
        .unwrap();
        assert_eq!(vnd_deal.currency, Some("VND".to_string()));

        // Test create_deal_internal rejecting invalid currency
        let bad_deal_err = commands::create_deal_internal(
            &conn,
            new_lead_id,
            "Bad Currency Deal".to_string(),
            100.0,
            "Prospecting".to_string(),
            Some("EUR".to_string()),
        );
        assert!(bad_deal_err.is_err(), "Must reject invalid currency EUR");

        // Test add_lead_with_currency_internal with USD
        let usd_lead_id = commands::add_lead_with_currency_internal(
            &mut conn,
            "Global Client".to_string(),
            "Global Corp".to_string(),
            "Qualified".to_string(),
            2500.0,
            "Alex Rivera".to_string(),
            "global@client.com".to_string(),
            "+1998877".to_string(),
            None,
            None,
            Some("USD".to_string()),
        )
        .unwrap();
        let usd_lead = commands::get_lead_details_internal(&conn, usd_lead_id).unwrap();
        assert_eq!(usd_lead.currency, Some("USD".to_string()));
        assert_eq!(usd_lead.value, 2500.0);

        // Test add_lead_with_currency_internal rejecting invalid currency
        let bad_lead_err = commands::add_lead_with_currency_internal(
            &mut conn,
            "Bad Lead".to_string(),
            "Bad Corp".to_string(),
            "New".to_string(),
            100.0,
            "Alex Rivera".to_string(),
            "bad@lead.com".to_string(),
            "+1998877".to_string(),
            None,
            None,
            Some("GBP".to_string()),
        );
        assert!(
            bad_lead_err.is_err(),
            "Must reject invalid currency GBP on add_lead"
        );

        // Test update_lead_with_currency_internal updating to USD
        commands::update_lead_with_currency_internal(
            &conn,
            new_lead_id,
            "John Doe Updated".to_string(),
            "Example Inc".to_string(),
            "Proposal".to_string(),
            3000.0,
            "Alex Rivera".to_string(),
            "john@example.com".to_string(),
            "+11223344".to_string(),
            None,
            None,
            Some("USD".to_string()),
        )
        .unwrap();
        let updated_lead = commands::get_lead_details_internal(&conn, new_lead_id).unwrap();
        assert_eq!(updated_lead.currency, Some("USD".to_string()));
        assert_eq!(updated_lead.value, 3000.0);

        // Test update_lead_with_currency_internal preserving existing USD currency when currency is None
        commands::update_lead_with_currency_internal(
            &conn,
            new_lead_id,
            "John Doe Name Changed".to_string(),
            "Example Inc".to_string(),
            "Proposal".to_string(),
            3500.0,
            "Alex Rivera".to_string(),
            "john@example.com".to_string(),
            "+11223344".to_string(),
            None,
            None,
            None,
        )
        .unwrap();
        let preserved_lead = commands::get_lead_details_internal(&conn, new_lead_id).unwrap();
        assert_eq!(
            preserved_lead.currency,
            Some("USD".to_string()),
            "Omitted currency in update must preserve existing USD currency"
        );
        assert_eq!(preserved_lead.value, 3500.0);

        // Test update_lead_with_currency_internal rejecting invalid currency
        let bad_update_err = commands::update_lead_with_currency_internal(
            &conn,
            new_lead_id,
            "John Doe".to_string(),
            "Example Inc".to_string(),
            "Proposal".to_string(),
            3000.0,
            "Alex Rivera".to_string(),
            "john@example.com".to_string(),
            "+11223344".to_string(),
            None,
            None,
            Some("AUD".to_string()),
        );
        assert!(
            bad_update_err.is_err(),
            "Must reject invalid currency AUD on update_lead"
        );

        // Test get_currency_settings_internal default
        let default_settings = commands::get_currency_settings_internal(&conn).unwrap();
        assert_eq!(default_settings.base_currency, "VND");
        assert_eq!(default_settings.exchange_rate, 25400.0);

        // Test update_currency_settings_internal
        let updated_settings =
            commands::update_currency_settings_internal(&mut conn, "USD".to_string(), 25000.0)
                .unwrap();
        assert_eq!(updated_settings.base_currency, "USD");
        assert_eq!(updated_settings.exchange_rate, 25000.0);

        let retrieved_settings = commands::get_currency_settings_internal(&conn).unwrap();
        assert_eq!(retrieved_settings.base_currency, "USD");
        assert_eq!(retrieved_settings.exchange_rate, 25000.0);

        // Test update_currency_settings_internal rejecting invalid rate or currency
        assert!(
            commands::update_currency_settings_internal(&mut conn, "EUR".to_string(), 25000.0)
                .is_err()
        );
        assert!(
            commands::update_currency_settings_internal(&mut conn, "USD".to_string(), -10.0)
                .is_err()
        );
        assert!(
            commands::update_currency_settings_internal(&mut conn, "USD".to_string(), 0.0).is_err()
        );

        // Ensure rejected updates did not modify existing settings (atomic validation check)
        let uncorrupted_settings = commands::get_currency_settings_internal(&conn).unwrap();
        assert_eq!(uncorrupted_settings.base_currency, "USD");
        assert_eq!(uncorrupted_settings.exchange_rate, 25000.0);

        // Test true SQLite transaction rollback when second statement fails mid-transaction
        conn.execute(
            "CREATE TRIGGER fail_second_fx_write BEFORE INSERT ON app_metadata WHEN NEW.key = 'usd_vnd_exchange_rate' BEGIN SELECT RAISE(ABORT, 'forced mid-transaction second write failure'); END;",
            [],
        )
        .unwrap();

        let mid_tx_failure =
            commands::update_currency_settings_internal(&mut conn, "VND".to_string(), 24000.0);
        assert!(
            mid_tx_failure.is_err(),
            "Must fail when second SQL statement errors mid-transaction"
        );

        conn.execute("DROP TRIGGER fail_second_fx_write;", [])
            .unwrap();

        // Verify base_currency was rolled back and not left at "VND"
        let rolled_back_settings = commands::get_currency_settings_internal(&conn).unwrap();
        assert_eq!(
            rolled_back_settings.base_currency, "USD",
            "First key (base_currency) must roll back if second key fails"
        );
        assert_eq!(
            rolled_back_settings.exchange_rate, 25000.0,
            "Second key must remain uncorrupted"
        );

        let deals_after_create = commands::get_deals_internal(&conn).unwrap();
        assert_eq!(deals_after_create.len(), 4);

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
    fn zalo_api_options_do_not_start_realtime_listener() {
        let options = commands::zalo_api_options();

        assert!(!options.auto_start_listener);
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
            zalo_uid: None,
            channel_account_id: None,
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
        use crate::host::mock::MockApp;

        let conn = db::init_db(":memory:").unwrap();
        let db_state = commands::DbState {
            conn: std::sync::Mutex::new(conn),
        };

        let app = MockApp::new().manage(db_state);

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

        {
            let conn = state.conn.lock().unwrap();
            db::save_zalo_session(&conn, "imei-invalid", "MockUA", "not valid json", "vi").unwrap();
        }

        let status = commands::verify_zalo_session("imei-invalid".to_string(), state.clone())
            .await
            .unwrap();
        assert!(!status.is_valid);

        let conn = state.conn.lock().unwrap();
        let (auth_status, auth_checked_at, auth_error): (String, Option<String>, Option<String>) =
            conn.query_row(
                "SELECT auth_status, auth_checked_at, auth_error FROM zalo_sessions WHERE imei = ?1",
                ["imei-invalid"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(auth_status, "expired");
        assert!(auth_checked_at.is_some());
        assert!(auth_error
            .as_deref()
            .is_some_and(|error| error.contains("Invalid cookies format")));

        let runtime_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM channel_accounts WHERE provider_account_key = ?1",
                ["imei-invalid"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(runtime_rows, 0);
    }

    #[test]
    fn managed_zalo_accounts_redact_credentials() {
        let conn = db::init_db(":memory:").unwrap();
        conn.execute(
            "INSERT INTO zalo_sessions
             (imei, user_agent, cookies_json, language, display_name, auth_status)
             VALUES (?1, ?2, ?3, 'vi', ?4, 'valid')",
            rusqlite::params![
                "imei-secret",
                "secret-user-agent",
                "secret-cookies",
                "Hello World"
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO channel_accounts
             (channel, provider_account_key, display_name, connection_status, capabilities_json, created_at, updated_at)
             VALUES ('zalo', ?1, ?2, 'connected', '{}', ?3, ?3)",
            rusqlite::params!["imei-secret", "Hello World", "2026-08-01T00:00:00.000Z"],
        )
        .unwrap();

        let accounts = commands::get_managed_zalo_accounts_internal(&conn).unwrap();
        let json = serde_json::to_string(&accounts).unwrap();

        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].runtime_status, "connected");
        assert!(!json.contains("secret-cookies"));
        assert!(!json.contains("secret-user-agent"));
        assert!(!json.contains("cookies_json"));
        assert!(!json.contains("user_agent"));
    }

    #[test]
    fn managed_zalo_accounts_deduplicates_by_account() {
        let conn = db::init_db(":memory:").unwrap();
        // Insert multiple sessions for same UID
        conn.execute(
            "INSERT INTO zalo_sessions (imei, user_agent, cookies_json, language, zalo_uid, display_name, auth_status)
             VALUES ('imei-1', 'UA1', '[]', 'vi', 'uid-multi', 'Old Name', 'expired')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO zalo_sessions (imei, user_agent, cookies_json, language, zalo_uid, display_name, auth_status)
             VALUES ('imei-2', 'UA2', '[]', 'vi', 'uid-multi', 'Active Name', 'valid')",
            [],
        ).unwrap();

        // Insert TWO channel_accounts rows that both match the session (one keyed by UID, one by IMEI)
        conn.execute(
            "INSERT INTO channel_accounts (channel, provider_account_key, display_name, connection_status, capabilities_json, created_at, updated_at)
             VALUES ('zalo', 'uid-multi', 'Active Name', 'connected', '{}', '2026-08-20T00:00:00Z', '2026-08-20T00:00:00Z')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO channel_accounts (channel, provider_account_key, display_name, connection_status, capabilities_json, created_at, updated_at)
             VALUES ('zalo', 'imei-2', 'Old Keyed Name', 'disconnected', '{}', '2026-08-19T00:00:00Z', '2026-08-19T00:00:00Z')",
            [],
        ).unwrap();

        // Also insert two pending empty-string UID sessions
        conn.execute(
            "INSERT INTO zalo_sessions (imei, user_agent, cookies_json, language, zalo_uid, display_name, auth_status)
             VALUES ('imei-empty-1', 'UA', '[]', 'vi', '', 'Pending 1', 'valid')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO zalo_sessions (imei, user_agent, cookies_json, language, zalo_uid, display_name, auth_status)
             VALUES ('imei-empty-2', 'UA', '[]', 'vi', '  ', 'Pending 2', 'valid')",
            [],
        ).unwrap();

        let accounts = commands::get_managed_zalo_accounts_internal(&conn).unwrap();
        assert_eq!(
            accounts.len(),
            3,
            "Must return 1 deduped account for uid-multi and 2 distinct accounts for empty UIDs"
        );

        let multi = accounts
            .iter()
            .find(|a| a.account_key == "uid-multi")
            .unwrap();
        assert_eq!(multi.display_name.as_deref(), Some("Active Name"));
        assert_eq!(multi.auth_status, "valid");
        assert_eq!(multi.runtime_status, "connected");

        assert!(accounts.iter().any(|a| a.account_key == "imei-empty-1"));
        assert!(accounts.iter().any(|a| a.account_key == "imei-empty-2"));
    }

    #[test]
    fn managed_smtp_accounts_redact_password() {
        let conn = db::init_db(":memory:").unwrap();
        db::save_smtp_server(
            &conn,
            &crate::models::SmtpServer {
                id: None,
                name: "Primary SMTP".to_string(),
                host: "smtp.example.com".to_string(),
                port: 587,
                username: "sender@example.com".to_string(),
                password: "secret-password".to_string(),
                from_email: "sender@example.com".to_string(),
                from_name: "Sales".to_string(),
                security: "STARTTLS".to_string(),
                created_at: String::new(),
            },
        )
        .unwrap();

        let accounts = commands::get_managed_smtp_accounts_internal(&conn).unwrap();
        let json = serde_json::to_string(&accounts).unwrap();

        assert_eq!(accounts.len(), 1);
        assert!(!json.contains("secret-password"));
        assert!(!json.contains("password"));
    }

    #[test]
    fn test_facebook_managed_accounts_list_and_deletion() {
        let conn = db::init_db(":memory:").unwrap();

        // 1. Insert channel account for Facebook Messenger
        conn.execute(
            "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, avatar_url, connection_status, created_at, updated_at)
             VALUES (101, 'messenger', 'page_123', 'RustSale Page', 'https://example.com/page.png', 'connected', datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        // 2. Insert page token
        conn.execute(
            "INSERT INTO facebook_page_tokens (channel_account_id, page_id, access_token_encrypted, token_status, granted_scopes_json, created_at, updated_at)
             VALUES (101, 'page_123', 'enc_token_abc', 'valid', '[\"pages_messaging\",\"pages_manage_posts\"]', datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        // 3. Insert historical conversation and message
        conn.execute(
            "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, title, created_at, updated_at)
             VALUES (201, 101, 'thread_fb_1', 'direct', 'Customer Chat', datetime('now'), datetime('now'))",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO messages (id, conversation_id, direction, content_type, text_content, ingested_at)
             VALUES (301, 201, 'inbound', 'text', 'Hello Facebook', datetime('now'))",
            [],
        ).unwrap();

        // 4. Insert scheduled posts across various lifecycles: queued, scheduled_on_meta, publishing (leased), delivery_unknown
        conn.execute(
            "INSERT INTO facebook_scheduled_posts (id, draft_group_id, channel_account_id, page_id, post_type, message, media_paths_json, scheduling_mode, idempotency_key, status, created_at, updated_at)
             VALUES (401, 'grp_1', 101, 'page_123', 'status', 'Queued Post', '[]', 'local_queued', 'idem_1', 'queued', datetime('now'), datetime('now'))",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO facebook_scheduled_posts (id, draft_group_id, channel_account_id, page_id, post_type, message, media_paths_json, scheduling_mode, remote_post_id, idempotency_key, status, created_at, updated_at)
             VALUES (402, 'grp_2', 101, 'page_123', 'status', 'Meta Scheduled Post', '[]', 'meta', 'remote_meta_999', 'idem_2', 'scheduled_on_meta', datetime('now'), datetime('now'))",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO facebook_scheduled_posts (id, draft_group_id, channel_account_id, page_id, post_type, message, media_paths_json, scheduling_mode, owner_id, claim_deadline, idempotency_key, status, created_at, updated_at)
             VALUES (403, 'grp_3', 101, 'page_123', 'status', 'In Flight Post', '[]', 'meta', 'worker_xyz', datetime('now', '+5 minutes'), 'idem_3', 'publishing', datetime('now'), datetime('now'))",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO facebook_scheduled_posts (id, draft_group_id, channel_account_id, page_id, post_type, message, media_paths_json, scheduling_mode, idempotency_key, status, created_at, updated_at)
             VALUES (404, 'grp_4', 101, 'page_123', 'status', 'Ambiguous Post', '[]', 'meta', 'idem_4', 'delivery_unknown', datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        // 5. Insert active upload session
        conn.execute(
            "INSERT INTO facebook_upload_sessions (id, post_id, page_id, upload_type, upload_session_id, file_path, file_hash, file_size_bytes, status, owner_id, lease_deadline, created_at, updated_at)
             VALUES (501, 403, 'page_123', 'video', 'sess_abc', '/tmp/vid.mp4', 'hash123', 1024, 'in_progress', 'upload_worker_1', datetime('now', '+5 minutes'), datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        // 6. List managed accounts
        let accounts = commands::get_managed_facebook_accounts_internal(&conn).unwrap();
        assert_eq!(accounts.len(), 1);
        let acc = &accounts[0];
        assert_eq!(acc.page_id, "page_123");
        assert_eq!(acc.page_name.as_deref(), Some("RustSale Page"));
        assert_eq!(
            acc.avatar_url.as_deref(),
            Some("https://example.com/page.png")
        );
        assert_eq!(acc.token_status, "valid");
        assert_eq!(acc.runtime_status, "connected");
        assert_eq!(
            acc.granted_scopes,
            vec!["pages_messaging", "pages_manage_posts"]
        );

        // 7. Disconnect account (non-destructive)
        commands::delete_facebook_account_internal(&conn, "page_123").unwrap();

        // 8. Verify no longer returned in managed accounts list (token removed)
        let remaining = commands::get_managed_facebook_accounts_internal(&conn).unwrap();
        assert_eq!(remaining.len(), 0);

        // 9. Verify historical conversation and messages were NOT deleted
        let conv_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM channel_conversations WHERE id = 201",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            conv_count, 1,
            "Historical conversations must be preserved upon disconnect"
        );

        let msg_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM messages WHERE id = 301", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            msg_count, 1,
            "Historical messages must be preserved upon disconnect"
        );

        // 10. Verify channel account is marked disconnected
        let ca_status: String = conn
            .query_row(
                "SELECT connection_status FROM channel_accounts WHERE id = 101",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ca_status, "disconnected");

        // 11. Verify active upload session is failed and leases cleared
        let (upload_status, upload_owner, upload_lease): (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT status, owner_id, lease_deadline FROM facebook_upload_sessions WHERE id = 501",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(upload_status, "failed", "Upload session must be failed");
        assert!(upload_owner.is_none(), "Upload owner lease must be cleared");
        assert!(
            upload_lease.is_none(),
            "Upload lease deadline must be cleared"
        );

        // 12. Verify queued, scheduled_on_meta, publishing (lease cleared), and delivery_unknown posts are all marked failed with error_code
        for post_id in [401, 402, 403, 404] {
            let (status, error_code, owner_id, claim_deadline): (
                String,
                Option<String>,
                Option<String>,
                Option<String>,
            ) = conn
                .query_row(
                    "SELECT status, error_code, owner_id, claim_deadline FROM facebook_scheduled_posts WHERE id = ?1",
                    [post_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .unwrap();
            assert_eq!(status, "failed", "Post {} must be failed", post_id);
            assert_eq!(
                error_code.as_deref(),
                Some("page_disconnected"),
                "Post {} must have error_code page_disconnected",
                post_id
            );
            assert!(
                owner_id.is_none(),
                "Post {} owner lease must be cleared",
                post_id
            );
            assert!(
                claim_deadline.is_none(),
                "Post {} claim deadline must be cleared",
                post_id
            );
        }
    }

    #[test]
    fn test_facebook_managed_accounts_handles_corrupted_scopes() {
        let conn = db::init_db(":memory:").unwrap();

        conn.execute(
            "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
             VALUES (102, 'messenger', 'page_bad_scopes', 'Corrupted Scope Page', 'connected', datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO facebook_page_tokens (channel_account_id, page_id, access_token_encrypted, token_status, granted_scopes_json, created_at, updated_at)
             VALUES (102, 'page_bad_scopes', 'enc_token', 'valid', 'INVALID_JSON_STRING', datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        let accounts = commands::get_managed_facebook_accounts_internal(&conn).unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].granted_scopes, vec!["invalid_scope_metadata"]);
    }

    #[test]
    fn test_save_managed_facebook_account_success() {
        let conn = db::init_db(":memory:").unwrap();

        let req = crate::models::SaveFacebookAccountRequest {
            page_id: "page_new_456".to_string(),
            page_name: Some("My Business Fanpage".to_string()),
            avatar_url: Some("https://example.com/avatar.jpg".to_string()),
            access_token: "EAA_test_access_token_12345".to_string(),
            granted_scopes: Some(vec![
                "pages_show_list".to_string(),
                "pages_messaging".to_string(),
                "pages_manage_posts".to_string(),
            ]),
        };

        let saved = commands::save_managed_facebook_account_internal(&conn, req).unwrap();
        assert_eq!(saved.page_id, "page_new_456");
        assert_eq!(saved.page_name.as_deref(), Some("My Business Fanpage"));
        assert_eq!(
            saved.avatar_url.as_deref(),
            Some("https://example.com/avatar.jpg")
        );
        assert_eq!(saved.token_status, "valid");
        assert_eq!(saved.runtime_status, "connected");
        assert_eq!(
            saved.granted_scopes,
            vec![
                "pages_show_list".to_string(),
                "pages_messaging".to_string(),
                "pages_manage_posts".to_string()
            ]
        );

        // Verify token in vault can be decrypted with the same channel_account_id
        let decrypted_token = crate::facebook::vault::FacebookTokenVault::get_valid_page_token(
            &conn,
            saved.channel_account_id,
            "page_new_456",
        )
        .unwrap();
        assert_eq!(decrypted_token, "EAA_test_access_token_12345");

        // Verify listing reflects the newly saved account
        let accounts = commands::get_managed_facebook_accounts_internal(&conn).unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].page_id, "page_new_456");
    }

    #[test]
    fn test_save_managed_facebook_account_validation() {
        let conn = db::init_db(":memory:").unwrap();

        // Empty page_id
        let err_page = commands::save_managed_facebook_account_internal(
            &conn,
            crate::models::SaveFacebookAccountRequest {
                page_id: "".to_string(),
                page_name: Some("Page".to_string()),
                avatar_url: None,
                access_token: "tok123".to_string(),
                granted_scopes: None,
            },
        )
        .unwrap_err();
        assert!(err_page.contains("Page ID cannot be empty"));

        // Empty access_token
        let err_token = commands::save_managed_facebook_account_internal(
            &conn,
            crate::models::SaveFacebookAccountRequest {
                page_id: "123".to_string(),
                page_name: Some("Page".to_string()),
                avatar_url: None,
                access_token: "   ".to_string(),
                granted_scopes: None,
            },
        )
        .unwrap_err();
        assert!(err_token.contains("Access Token cannot be empty"));
    }

    #[test]
    fn test_save_managed_facebook_account_defaults_to_minimal_scopes() {
        let conn = db::init_db(":memory:").unwrap();

        let req = crate::models::SaveFacebookAccountRequest {
            page_id: "page_minimal_111".to_string(),
            page_name: Some("Minimal Scopes Page".to_string()),
            avatar_url: None,
            access_token: "EAA_test_token_minimal".to_string(),
            granted_scopes: None,
        };

        let saved = commands::save_managed_facebook_account_internal(&conn, req).unwrap();
        assert_eq!(saved.page_id, "page_minimal_111");
        assert!(saved.granted_scopes.is_empty());
    }

    #[test]
    fn test_map_facebook_tasks_to_scopes_exact_and_failclosed() {
        // 1. NPE Profile Plus Full Control grants full page management scopes
        let full_control_tasks = vec![serde_json::json!("PROFILE_PLUS_FULL_CONTROL")];
        let scopes = commands::map_facebook_tasks_to_scopes(&full_control_tasks);
        assert!(scopes.contains(&"pages_show_list".to_string()));
        assert!(scopes.contains(&"pages_messaging".to_string()));
        assert!(scopes.contains(&"pages_manage_posts".to_string()));
        assert!(scopes.contains(&"pages_read_engagement".to_string()));

        // 2. Exact NPE granular tasks
        let npe_tasks = vec![
            serde_json::json!("PROFILE_PLUS_MESSAGING"),
            serde_json::json!("PROFILE_PLUS_CREATE_CONTENT"),
            serde_json::json!("PROFILE_PLUS_ANALYZE"),
        ];
        let npe_scopes = commands::map_facebook_tasks_to_scopes(&npe_tasks);
        assert_eq!(
            npe_scopes,
            vec![
                "pages_show_list".to_string(),
                "pages_messaging".to_string(),
                "pages_manage_posts".to_string(),
                "pages_manage_engagement".to_string(),
                "pages_read_engagement".to_string(),
                "pages_read_user_content".to_string(),
                "leads_retrieval".to_string(),
            ]
        );

        // 3. Legacy task names
        let legacy_tasks = vec![
            serde_json::json!("MESSAGING"),
            serde_json::json!("MANAGE"),
            serde_json::json!("MODERATE"),
        ];
        let legacy_scopes = commands::map_facebook_tasks_to_scopes(&legacy_tasks);
        assert_eq!(
            legacy_scopes,
            vec![
                "pages_show_list".to_string(),
                "pages_messaging".to_string(),
                "pages_manage_posts".to_string(),
                "pages_manage_engagement".to_string(),
                "pages_read_engagement".to_string(),
                "pages_read_user_content".to_string(),
                "leads_retrieval".to_string(),
            ]
        );

        // 4. Unknown task names and lookalikes fail closed strictly to pages_show_list
        let unknown_tasks = vec![
            serde_json::json!("PROFILE_PLUS_NOT_MESSAGING"),
            serde_json::json!("UNKNOWN_FUTURE_CAPABILITY"),
            serde_json::json!("MANAGE_PAYMENTS_ONLY"),
        ];
        let unknown_scopes = commands::map_facebook_tasks_to_scopes(&unknown_tasks);
        assert_eq!(unknown_scopes, vec!["pages_show_list".to_string()]);
    }

    #[test]
    fn test_parse_meta_app_permissions() {
        let json = serde_json::json!({
            "data": [
                { "permission": "pages_show_list", "status": "granted" },
                { "permission": "pages_messaging", "status": "granted" },
                { "permission": "pages_manage_posts", "status": "declined" },
                { "permission": "user_friends", "status": "expired" },
            ]
        });

        let perms = commands::parse_meta_app_permissions(&json);
        assert_eq!(perms, vec!["pages_show_list", "pages_messaging"]);

        // Malformed, empty, or all declined JSON returns empty
        assert!(commands::parse_meta_app_permissions(&serde_json::json!({})).is_empty());
        assert!(commands::parse_meta_app_permissions(&serde_json::json!({"data": []})).is_empty());
    }

    #[test]
    fn test_intersect_tasks_and_app_permissions_enforces_dual_authorization() {
        let full_task_scopes = vec![
            "pages_show_list".to_string(),
            "pages_messaging".to_string(),
            "pages_manage_posts".to_string(),
            "pages_read_engagement".to_string(),
        ];

        // 1. App only granted pages_messaging and pages_show_list: effective scopes MUST ONLY include those two
        let app_perms_messaging_only =
            vec!["pages_show_list".to_string(), "pages_messaging".to_string()];
        let effective1 = commands::intersect_tasks_and_app_permissions(
            &full_task_scopes,
            &app_perms_messaging_only,
        );
        assert_eq!(
            effective1,
            vec!["pages_show_list".to_string(), "pages_messaging".to_string()]
        );

        // 2. App granted all permissions, but Page task only grants pages_messaging:
        let messaging_task_only =
            vec!["pages_show_list".to_string(), "pages_messaging".to_string()];
        let app_perms_all = vec![
            "pages_show_list".to_string(),
            "pages_messaging".to_string(),
            "pages_manage_posts".to_string(),
            "pages_read_engagement".to_string(),
        ];
        let effective2 =
            commands::intersect_tasks_and_app_permissions(&messaging_task_only, &app_perms_all);
        assert_eq!(
            effective2,
            vec!["pages_show_list".to_string(), "pages_messaging".to_string()]
        );

        // 3. App permissions missing pages_show_list: strict mathematical intersection does not include pages_show_list
        let app_perms_no_show_list = vec!["pages_messaging".to_string()];
        let effective3 = commands::intersect_tasks_and_app_permissions(
            &full_task_scopes,
            &app_perms_no_show_list,
        );
        assert_eq!(effective3, vec!["pages_messaging".to_string()]);

        // 4. Empty app permissions (e.g. API/network failure, zero grants): strictly empty (fails closed)
        let effective_empty = commands::intersect_tasks_and_app_permissions(&full_task_scopes, &[]);
        assert!(effective_empty.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_facebook_pages_empty_token_validation() {
        let err = commands::fetch_facebook_pages_from_token_internal("   ", None)
            .await
            .unwrap_err();
        assert!(err.contains("Access token cannot be empty"));
    }

    #[test]
    fn test_map_facebook_tasks_to_scopes_comprehensive() {
        let full_control_tasks = vec![serde_json::json!("PROFILE_PLUS_FULL_CONTROL")];
        let scopes = commands::map_facebook_tasks_to_scopes(&full_control_tasks);
        assert!(scopes.contains(&"pages_show_list".to_string()));
        assert!(scopes.contains(&"pages_messaging".to_string()));
        assert!(scopes.contains(&"pages_manage_posts".to_string()));
        assert!(scopes.contains(&"pages_read_engagement".to_string()));
        assert!(scopes.contains(&"pages_manage_engagement".to_string()));
        assert!(scopes.contains(&"pages_read_user_content".to_string()));
        assert!(scopes.contains(&"leads_retrieval".to_string()));
    }

    #[test]
    fn test_parse_meta_token_exchange_response_success_and_error() {
        // Success case
        let success_json = serde_json::json!({
            "access_token": "EAAXxLongLivedToken123456",
            "token_type": "bearer",
            "expires_in": 5184000
        });
        let token = commands::parse_meta_token_exchange_response(&success_json).unwrap();
        assert_eq!(token, "EAAXxLongLivedToken123456");

        // Error case from Meta API
        let error_json = serde_json::json!({
            "error": {
                "message": "Error validating client secret.",
                "type": "OAuthException",
                "code": 1
            }
        });
        let err = commands::parse_meta_token_exchange_response(&error_json).unwrap_err();
        assert!(err.contains("Error validating client secret"));

        // Malformed / Empty response
        let empty_json = serde_json::json!({});
        let err2 = commands::parse_meta_token_exchange_response(&empty_json).unwrap_err();
        assert!(err2.contains("Invalid response"));
    }

    #[test]
    fn test_load_dotenv_if_present_and_env_resolution() {
        crate::load_dotenv_if_present();
        // Check that calling load_dotenv_if_present multiple times is idempotent and safe
        crate::load_dotenv_if_present();
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

    #[test]
    fn test_require_zalo_uid_rejects_missing_identity_and_normalizes_valid_uid() {
        let missing_error = commands::require_zalo_uid(None).unwrap_err();
        assert!(missing_error.contains("account UID could not be determined"));

        let blank_error = commands::require_zalo_uid(Some("   ".to_string())).unwrap_err();
        assert_eq!(blank_error, missing_error);

        assert!(commands::require_zalo_uid(Some("0".to_string())).is_err());
        assert!(commands::require_zalo_uid(Some("-1".to_string())).is_err());
        assert!(commands::require_zalo_uid(Some("null".to_string())).is_err());
        assert!(commands::require_zalo_uid(Some("undefined".to_string())).is_err());

        assert_eq!(
            commands::require_zalo_uid(Some(" 1503088233761505331 ".to_string())).unwrap(),
            "1503088233761505331"
        );

        assert_eq!(commands::sanitize_valid_zalo_uid(None), None);
        assert_eq!(commands::sanitize_valid_zalo_uid(Some("   ")), None);
        assert_eq!(commands::sanitize_valid_zalo_uid(Some("0")), None);
        assert_eq!(commands::sanitize_valid_zalo_uid(Some("-1")), None);
        assert_eq!(commands::sanitize_valid_zalo_uid(Some("NULL")), None);
        assert_eq!(commands::sanitize_valid_zalo_uid(Some("undefined")), None);
        assert_eq!(
            commands::sanitize_valid_zalo_uid(Some(" 9876543210 ")),
            Some("9876543210".to_string())
        );
    }

    #[test]
    fn test_zalo_qr_persistence_rollback_restores_existing_session_or_removes_new_one() {
        let conn = db::init_db(":memory:").unwrap();
        conn.execute(
            "INSERT INTO channel_accounts
             (id, channel, provider_account_key, self_external_id, display_name, connection_status, capabilities_json, created_at, updated_at)
             VALUES (55, 'zalo', 'uid-55', 'uid-55', 'Old Account', 'connected', '{}', '2026-08-09T00:00:00Z', '2026-08-09T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO zalo_sessions
             (imei, user_agent, cookies_json, language, display_name, avatar, auth_status, auth_checked_at, auth_error, zalo_uid, channel_account_id)
             VALUES ('imei-existing', 'old-agent', '[{\"old\":true}]', 'vi', 'Old Account', 'old.png', 'valid', '2026-08-09T00:00:00Z', NULL, 'uid-55', 55)",
            [],
        )
        .unwrap();

        let existing_snapshot =
            commands::capture_zalo_session_snapshot(&conn, "imei-existing").unwrap();
        db::save_zalo_session(
            &conn,
            "imei-existing",
            "new-agent",
            "[{\"new\":true}]",
            "vi",
        )
        .unwrap();
        db::update_zalo_session_profile(&conn, "imei-existing", Some("New Account"), None).unwrap();
        commands::rollback_zalo_session_persistence(&conn, "imei-existing", existing_snapshot)
            .unwrap();

        let restored: (String, String, Option<String>, Option<String>, Option<String>, Option<i64>) =
            conn.query_row(
                "SELECT user_agent, cookies_json, display_name, avatar, zalo_uid, channel_account_id
                 FROM zalo_sessions WHERE imei = 'imei-existing'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            restored,
            (
                "old-agent".to_string(),
                "[{\"old\":true}]".to_string(),
                Some("Old Account".to_string()),
                Some("old.png".to_string()),
                Some("uid-55".to_string()),
                Some(55),
            )
        );

        let new_snapshot = commands::capture_zalo_session_snapshot(&conn, "imei-new").unwrap();
        db::save_zalo_session(&conn, "imei-new", "new-agent", "[]", "vi").unwrap();
        commands::rollback_zalo_session_persistence(&conn, "imei-new", new_snapshot).unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM zalo_sessions WHERE imei = 'imei-new'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn test_send_zalo_outreach_mock() {
        use std::sync::Mutex;

        let db_path = std::env::temp_dir().join(format!(
            "rustsale-cmd-outreach-test-{}.db",
            rand::random::<u64>()
        ));
        let db_str = db_path.to_str().unwrap().to_string();
        struct TestCleanup(std::path::PathBuf);
        impl Drop for TestCleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
                let _ = std::fs::remove_file(self.0.with_extension("db-wal"));
                let _ = std::fs::remove_file(self.0.with_extension("db-shm"));
            }
        }
        let _cleanup = TestCleanup(db_path.clone());

        let mut conn = db::init_db(&db_str).unwrap();
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
            NO_APP_HANDLE,
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

        let repository =
            crate::conversation::repository::ConversationRepository::new(db_str.clone());
        let page = repository
            .get_contact_timeline(
                lead_id_1 as i64,
                &[crate::conversation::types::Channel::Zalo],
                None,
                50,
            )
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert!(page
            .items
            .iter()
            .all(|message| message.direction
                == crate::conversation::types::MessageDirection::Outbound));
    }

    #[tokio::test]
    async fn test_send_zalo_outreach_with_cached_uid_and_uid_persistence() {
        use std::sync::Mutex;

        let db_path = std::env::temp_dir().join(format!(
            "rustsale-cmd-cached-uid-test-{}.db",
            rand::random::<u64>()
        ));
        let db_str = db_path.to_str().unwrap().to_string();
        struct TestCleanup(std::path::PathBuf);
        impl Drop for TestCleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
                let _ = std::fs::remove_file(self.0.with_extension("db-wal"));
                let _ = std::fs::remove_file(self.0.with_extension("db-shm"));
            }
        }
        let _cleanup = TestCleanup(db_path.clone());

        let mut conn = db::init_db(&db_str).unwrap();
        db::seed_data(&mut conn).unwrap();

        let template_id =
            db::create_outreach_template(&conn, "Cached UID Template", "Hello [name]!", None)
                .unwrap();

        // Lead 1: has pre-cached zalo_uid, phone is "not-found" AND zalo_status was previously 'VERIFIED_NOT_EXIST'.
        // Valid UID MUST be prioritized and override the old lookup failure!
        let lead_id_1 = commands::add_lead_internal(
            &mut conn,
            "Cached User".to_string(),
            "Cached Corp".to_string(),
            "New".to_string(),
            1000.0,
            "Alex".to_string(),
            "cached@example.com".to_string(),
            "not-found".to_string(),
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE leads SET zalo_uid = 'cached-uid-99999', zalo_status = 'VERIFIED_NOT_EXIST' WHERE id = ?1",
            rusqlite::params![lead_id_1],
        )
        .unwrap();

        // Lead 2: has invalid zalo_uid ('0'), valid phone.
        // Invalid UID MUST be rejected by sanitize_valid_zalo_uid, falling back to phone lookup and persisting the new UID.
        let lead_id_2 = commands::add_lead_internal(
            &mut conn,
            "Uncached User".to_string(),
            "Uncached Corp".to_string(),
            "New".to_string(),
            2000.0,
            "Alex".to_string(),
            "uncached@example.com".to_string(),
            "0987654321".to_string(),
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE leads SET zalo_uid = '0' WHERE id = ?1",
            rusqlite::params![lead_id_2],
        )
        .unwrap();

        // Lead 3: has pre-cached zalo_uid, but zalo_status is 'BLOCKED_STRANGER'.
        // Privacy block MUST be respected and this lead skipped.
        let lead_id_3 = commands::add_lead_internal(
            &mut conn,
            "Blocked Stranger User".to_string(),
            "Blocked Corp".to_string(),
            "New".to_string(),
            3000.0,
            "Alex".to_string(),
            "blocked@example.com".to_string(),
            "0912345678".to_string(),
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE leads SET zalo_uid = 'blocked-uid-11111', zalo_status = 'BLOCKED_STRANGER' WHERE id = ?1",
            rusqlite::params![lead_id_3],
        )
        .unwrap();

        let db_mutex = Mutex::new(conn);

        let res = commands::send_zalo_outreach_internal(
            NO_APP_HANDLE,
            &db_mutex,
            "mock-imei".to_string(),
            vec![lead_id_1, lead_id_2, lead_id_3],
            template_id,
            commands::ZaloOutreachRunOptions::default(),
            commands::OutreachProgressScope::default(),
        )
        .await;

        // Total sent is 2 (lead 1 sent via cached UID overriding VERIFIED_NOT_EXIST, lead 2 sent via phone fallback, lead 3 skipped due to BLOCKED_STRANGER)
        assert_eq!(res.unwrap().total_sent, 2);

        let conn_locked = db_mutex.lock().unwrap();

        // Check Lead 1: Sent using cached UID despite old VERIFIED_NOT_EXIST status
        let lead_1 = commands::get_lead_details_internal(&conn_locked, lead_id_1).unwrap();
        assert_eq!(lead_1.zalo_reached, 1);
        assert_eq!(lead_1.has_zalo, Some(1));
        assert_eq!(lead_1.zalo_uid, Some("cached-uid-99999".to_string()));

        // Check Lead 2: Invalid '0' UID was rejected, sent via phone lookup, and valid UID was persisted
        let lead_2 = commands::get_lead_details_internal(&conn_locked, lead_id_2).unwrap();
        assert_eq!(lead_2.zalo_reached, 1);
        assert_eq!(lead_2.has_zalo, Some(1));
        assert_eq!(lead_2.zalo_uid, Some("mock-uid-0987654321".to_string()));

        // Check Lead 3: Skipped due to BLOCKED_STRANGER
        let lead_3 = commands::get_lead_details_internal(&conn_locked, lead_id_3).unwrap();
        assert_eq!(lead_3.zalo_reached, 0);

        // Check channel_conversations table has both external_thread_ids recorded
        let mut stmt = conn_locked
            .prepare("SELECT external_thread_id FROM channel_conversations ORDER BY id ASC")
            .unwrap();
        let recipients: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(recipients, vec!["cached-uid-99999", "mock-uid-0987654321"]);
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
            NO_APP_HANDLE,
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
        use crate::host::mock::MockApp;

        let conn = db::init_db(":memory:").unwrap();
        let mut conn = conn;
        db::seed_data(&mut conn).unwrap();

        let db_state = commands::DbState {
            conn: std::sync::Mutex::new(conn),
        };

        let app = MockApp::new().manage(db_state);

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
        let db_path = std::env::temp_dir().join(format!(
            "rustsale-cmd-campaign-test-{}.db",
            rand::random::<u64>()
        ));
        let db_str = db_path.to_str().unwrap().to_string();
        struct TestCleanup(std::path::PathBuf);
        impl Drop for TestCleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
                let _ = std::fs::remove_file(self.0.with_extension("db-wal"));
                let _ = std::fs::remove_file(self.0.with_extension("db-shm"));
            }
        }
        let _cleanup = TestCleanup(db_path.clone());

        let mut conn = db::init_db(&db_str).unwrap();
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

        let provider_key: String = conn_locked
            .query_row(
                "SELECT ca.provider_account_key FROM channel_accounts ca JOIN channel_conversations cc ON cc.channel_account_id = ca.id LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(provider_key, "mock-success-imei");

        let repository =
            crate::conversation::repository::ConversationRepository::new(db_str.clone());
        let page = repository
            .get_contact_timeline(
                lead_4 as i64,
                &[crate::conversation::types::Channel::Zalo],
                None,
                50,
            )
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert!(page
            .items
            .iter()
            .all(|message| message.direction
                == crate::conversation::types::MessageDirection::Outbound));
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

        let direct_conv_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM channel_conversations WHERE kind = 'direct'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(direct_conv_count, 2);

        let group_conv_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM channel_conversations WHERE kind = 'group'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(group_conv_count, 0);

        let total_messages: i64 = conn
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
            .unwrap();
        assert_eq!(total_messages, 2);

        let provider_keys: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT ca.provider_account_key FROM channel_accounts ca JOIN channel_conversations cc ON cc.channel_account_id = ca.id")
                .unwrap();
            stmt.query_map([], |row| row.get(0))
                .unwrap()
                .filter_map(Result::ok)
                .collect()
        };
        assert_eq!(provider_keys.len(), 2);
        assert!(provider_keys.iter().all(|k| k == "mock-success-imei"));
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

        let provider_key: String = conn_locked
            .query_row(
                "SELECT ca.provider_account_key FROM channel_accounts ca JOIN channel_conversations cc ON cc.channel_account_id = ca.id LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(provider_key, "mock-success-imei");
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
        use crate::host::mock::MockApp;
        use crate::models::SmtpServer;
        use tokio::net::TcpListener;

        let conn = db::init_db(":memory:").unwrap();
        let db_state = commands::DbState {
            conn: std::sync::Mutex::new(conn),
        };

        let app = MockApp::new().manage(db_state);

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
            currency: Some("VND".to_string()),
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

        // Create lead_4 with zalo_status = 'BLOCKED_STRANGER'
        let lead_4 = commands::add_lead_internal(
            &mut conn,
            "Blocked Stranger Lead".into(),
            "Inc".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "l4@test.com".into(),
            "0944444444".into(),
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE leads SET zalo_status = 'BLOCKED_STRANGER' WHERE id = ?1",
            [lead_4],
        )
        .unwrap();

        // Create lead_5 with zalo_status = 'VERIFIED_NOT_EXIST'
        let lead_5 = commands::add_lead_internal(
            &mut conn,
            "Not Exist Lead".into(),
            "Inc".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "l5@test.com".into(),
            "0955555555".into(),
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE leads SET zalo_status = 'VERIFIED_NOT_EXIST' WHERE id = ?1",
            [lead_5],
        )
        .unwrap();

        for lead_id in &[lead_1, lead_2, lead_3, lead_4, lead_5] {
            crate::db::add_lead_to_category(&conn, *lead_id, cat_id).unwrap();
        }

        // Create template
        let tpl_id = crate::db::create_outreach_template(&conn, "Test Tpl", "Hello", None).unwrap();

        // Insert outreach_history success for lead_3
        conn.execute(
            "INSERT INTO outreach_history (lead_id, template_id, status, sent_at, session_imei) VALUES (?1, ?2, 'SUCCESS', '2026-01-01T00:00:00Z', 'mock-imei')",
            rusqlite::params![lead_3, tpl_id],
        ).unwrap();

        let query = commands::OutreachLeadQuery {
            category_id: Some(cat_id),
            segmentation: "never_sent".to_string(),
            filter_template_id: None,
            channel: "zalo".to_string(),
            search: None,
            filter_verified_zalo: false,
            filter_unreached: false,
            filter_mobile_only: false,
        };

        let page = commands::get_outreach_leads_page_internal(&conn, &query, None, 50).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, Some(lead_1));
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
            NO_APP_HANDLE,
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
            NO_APP_HANDLE,
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

    #[tokio::test]
    async fn campaign_transaction_rolls_back_on_crm_error() {
        let temp_dir = std::env::temp_dir().join(format!(
            "test-crm-campaign-rollback-{}",
            rand::random::<u64>()
        ));
        let mut conn = db::init_db(temp_dir.to_str().unwrap()).unwrap();
        crate::conversation::migration::migrate(&conn).unwrap();

        let _lead_id = commands::add_lead_internal(
            &mut conn,
            "Rollback Lead".to_string(),
            "Corp".to_string(),
            "New".to_string(),
            100.0,
            "Owner".to_string(),
            "r@corp.com".to_string(),
            "0909000111".to_string(),
            Some("uid-rollback".to_string()),
            None,
        )
        .unwrap();

        // Create trigger on leads that fails UPDATE AFTER outreach_history INSERT has already succeeded inside tx
        conn.execute(
            "CREATE TRIGGER fail_lead_update AFTER UPDATE ON leads BEGIN SELECT RAISE(ABORT, 'Simulated CRM lead update error'); END;",
            [],
        )
        .unwrap();

        let db_mutex = std::sync::Arc::new(std::sync::Mutex::new(conn));

        let _ = commands::send_zalo_campaign_internal(
            None,
            &db_mutex,
            1,
            1,
            vec!["mock-campaign-1".to_string()],
        )
        .await;

        let conn_locked = db_mutex.lock().unwrap();

        // Outreach history must be 0 because the successful INSERT in step 1 was rolled back on step 2 failure!
        let history_count: i64 = conn_locked
            .query_row("SELECT COUNT(*) FROM outreach_history", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(history_count, 0);

        let conv_count: i64 = conn_locked
            .query_row("SELECT COUNT(*) FROM channel_conversations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(conv_count, 0);
    }

    #[test]
    fn test_group_member_export_and_batch_import() {
        let root = std::env::temp_dir().join(format!(
            "rustsale-group-member-command-import-{}",
            rand::random::<u64>()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let db_path = root.join("app.db");
        let conn = db::init_db(db_path.to_str().unwrap()).unwrap();
        let now = "2026-01-01T00:00:00Z";
        conn.execute(
            "INSERT INTO channel_accounts
             (id, channel, provider_account_key, display_name, created_at, updated_at)
             VALUES (1, 'zalo', 'key1', 'Zalo Acc 1', ?1, ?1)",
            [now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO channel_conversations
             (id, channel_account_id, external_thread_id, kind, title, created_at, updated_at)
             VALUES (100, 1, 'g100', 'group', 'Test Group', ?1, ?1)",
            [now],
        )
        .unwrap();
        for (identity_id, external_id, display_name, avatar_url, role) in [
            (
                1001_i64,
                "zalo_uid_1",
                "Member Alpha",
                "https://img/1.png",
                "admin",
            ),
            (
                1002_i64,
                "zalo_uid_2",
                "Member Beta",
                "https://img/2.png",
                "member",
            ),
        ] {
            conn.execute(
                "INSERT INTO channel_identities
                 (id, channel, identity_namespace, external_id, display_name, avatar_url,
                  created_at, updated_at)
                 VALUES (?1, 'zalo', 'zalo_uid', ?2, ?3, ?4, ?5, ?5)",
                rusqlite::params![identity_id, external_id, display_name, avatar_url, now],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO conversation_participants
                 (conversation_id, identity_id, participant_role, joined_at)
                 VALUES (100, ?1, ?2, ?3)",
                rusqlite::params![identity_id, role, now],
            )
            .unwrap();
        }

        let members = commands::fetch_group_members_impl(&conn, 100).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].display_name, "Member Alpha");
        assert_eq!(members[0].participant_role, "admin");
        drop(conn);

        let repository = std::sync::Arc::new(
            crate::conversation::repository::ConversationRepository::new(
                db_path.to_string_lossy().to_string(),
            ),
        );
        let events =
            std::sync::Arc::new(crate::conversation::events::ConversationEventPublisher::new(8));
        let service = crate::conversation::service::ConversationService::new(
            repository,
            events.clone(),
            root.join("spool"),
        );
        let mut receiver = events.subscribe();
        let result = commands::batch_import_group_members_to_leads_impl(
            &service,
            commands::BatchImportGroupMembersRequest {
                conversation_id: 100,
                members: vec![
                    commands::ImportMemberItem {
                        identity_id: Some(1001),
                        external_id: Some("zalo_uid_1".to_string()),
                        display_name: "Caller Name Must Not Authorize".to_string(),
                        avatar_url: Some("https://caller.invalid/1.png".to_string()),
                        phone: Some("0901111222".to_string()),
                    },
                    commands::ImportMemberItem {
                        identity_id: Some(1002),
                        external_id: Some("zalo_uid_2".to_string()),
                        display_name: "Caller Name Must Not Authorize".to_string(),
                        avatar_url: Some("https://caller.invalid/2.png".to_string()),
                        phone: None,
                    },
                ],
                category_ids: Vec::new(),
                new_category_name: Some("Vibe Coding Group Leads".to_string()),
                group_title: "Caller Group Title".to_string(),
                skip_existing: Some(true),
            },
        )
        .unwrap();
        assert_eq!(result.imported_count, 2);
        assert_eq!(result.updated_count, 0);
        assert_eq!(result.skipped_count, 0);
        assert_eq!(result.assigned_category_ids.len(), 1);

        let event = receiver.try_recv().unwrap();
        assert_eq!(event.conversation_id, 100);
        assert_eq!(
            event.reason,
            crate::conversation::events::ConversationInvalidationReason::Link
        );
        assert!(receiver.try_recv().is_err());

        let conn = crate::db::open_db_connection(db_path.to_str().unwrap()).unwrap();
        let updated_members = commands::fetch_group_members_impl(&conn, 100).unwrap();
        assert_eq!(updated_members.len(), 2);
        assert_eq!(updated_members[0].display_name, "Member Alpha");
        assert!(updated_members[0].lead_id.is_some());
        assert_eq!(updated_members[0].categories.len(), 1);
        assert_eq!(
            updated_members[0].categories[0].name,
            "Vibe Coding Group Leads"
        );
        drop(conn);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn group_member_export_includes_observed_sender_even_if_snapshot_marked_left() {
        let conn = db::init_db(":memory:").unwrap();
        let now = "2026-08-10T00:00:00Z";

        conn.execute(
            "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, created_at, updated_at)
             VALUES (1, 'zalo', 'key1', 'Zalo Acc 1', ?1, ?1)",
            [now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, title, created_at, updated_at)
             VALUES (100, 1, 'g100', 'group', 'Test Group', ?1, ?1)",
            [now],
        )
        .unwrap();

        for (id, external_id, display_name) in [
            (1001_i64, "owner_uid", "Owner"),
            (1002_i64, "self_uid", "Self"),
            (1003_i64, "observed_uid", "Observed Member"),
        ] {
            conn.execute(
                "INSERT INTO channel_identities
                 (id, channel, identity_namespace, external_id, display_name, created_at, updated_at)
                 VALUES (?1, 'zalo', 'zalo_uid', ?2, ?3, ?4, ?4)",
                rusqlite::params![id, external_id, display_name, now],
            )
            .unwrap();
        }

        conn.execute(
            "INSERT INTO conversation_participants
             (conversation_id, identity_id, participant_role, joined_at, left_at)
             VALUES (100, 1001, 'admin', ?1, NULL),
                    (100, 1002, 'member', ?1, NULL),
                    (100, 1003, 'member', ?1, ?1)",
            [now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO messages
             (conversation_id, sender_identity_id, direction, content_type, content_json, ingested_at, status)
             VALUES (100, 1003, 'inbound', 'text', '{}', ?1, 'received')",
            [now],
        )
        .unwrap();

        let members = commands::fetch_group_members_impl(&conn, 100).unwrap();

        assert_eq!(members.len(), 3);
        assert!(members
            .iter()
            .any(|member| member.external_id.as_deref() == Some("observed_uid")));
    }

    mod group_member_deep_scan {
        use crate::commands;
        use crate::conversation::events::ConversationEventPublisher;
        use crate::conversation::repository::{
            ConversationRepository, NormalizedGroupParticipant, NormalizedGroupSnapshot,
            NormalizedIdentity,
        };
        use crate::conversation::service::ConversationService;
        use crate::conversation::zalo::client::{
            GroupBoardUidReport, GroupBoardWarning, NormalizedProfile, ZaloClient,
        };
        use crate::conversation::zalo::supervisor::{
            GroupMessageBackfillOutcome, GroupMessageBackfillReport,
        };
        use crate::db;
        use async_trait::async_trait;
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::{Arc, Mutex};
        use tokio::sync::broadcast;
        use zca_rs::listener::ListenerEvent;
        use zca_rs::models::ThreadType;

        struct GroupScanHarness {
            root: PathBuf,
            db_path: PathBuf,
            state: commands::DbState,
            service: Arc<ConversationService>,
        }

        impl GroupScanHarness {
            fn new() -> Self {
                let root = std::env::temp_dir().join(format!(
                    "rustsale-group-member-deep-scan-{}",
                    rand::random::<u64>()
                ));
                std::fs::create_dir_all(&root).unwrap();
                let db_path = root.join("app.db");
                let conn = db::init_db(db_path.to_str().unwrap()).unwrap();
                let repository = Arc::new(ConversationRepository::new(
                    db_path.to_string_lossy().to_string(),
                ));
                let events = Arc::new(ConversationEventPublisher::new(16));
                let service = Arc::new(ConversationService::new(
                    repository,
                    events,
                    root.join("spool"),
                ));
                Self {
                    root,
                    db_path,
                    state: commands::DbState {
                        conn: Mutex::new(conn),
                    },
                    service,
                }
            }

            fn insert_conversation(&self, conversation_id: i64, kind: &str) {
                let conn = self.state.conn.lock().unwrap();
                let now = "2026-08-10T00:00:00Z";
                conn.execute(
                    "INSERT INTO channel_accounts
                     (id, channel, provider_account_key, display_name, created_at, updated_at)
                     VALUES (1, 'zalo', 'account-a', 'Account A', ?1, ?1)",
                    [now],
                )
                .unwrap();
                conn.execute(
                    "INSERT INTO channel_conversations
                     (id, channel_account_id, external_thread_id, kind, title, created_at, updated_at)
                     VALUES (?1, 1, 'group-a', ?2, 'Group A', ?3, ?3)",
                    rusqlite::params![conversation_id, kind, now],
                )
                .unwrap();
            }

            fn insert_local_participant(&self, conversation_id: i64, external_id: &str) {
                let conn = self.state.conn.lock().unwrap();
                let now = "2026-08-10T00:00:00Z";
                let identity_id = conn
                    .query_row(
                        "INSERT INTO channel_identities
                         (channel, identity_namespace, external_id, display_name, created_at, updated_at)
                         VALUES ('zalo', 'zalo_uid', ?1, ?2, ?3, ?3)
                         RETURNING id",
                        rusqlite::params![external_id, external_id, now],
                        |row| row.get::<_, i64>(0),
                    )
                    .unwrap();
                conn.execute(
                    "INSERT INTO conversation_participants
                     (conversation_id, identity_id, participant_role, joined_at)
                     VALUES (?1, ?2, 'member', ?3)",
                    rusqlite::params![conversation_id, identity_id, now],
                )
                .unwrap();
            }

            fn insert_whatsapp_conversation(
                &self,
                conversation_id: i64,
                kind: &str,
                thread_id: &str,
            ) {
                let conn = self.state.conn.lock().unwrap();
                let now = "2026-08-10T00:00:00Z";
                conn.execute(
                    "INSERT INTO channel_accounts
                     (id, channel, provider_account_key, display_name, created_at, updated_at)
                     VALUES (2, 'whatsapp', '84901234567', 'WhatsApp Acc', ?1, ?1)",
                    [now],
                )
                .unwrap();
                conn.execute(
                    "INSERT INTO channel_conversations
                     (id, channel_account_id, external_thread_id, kind, title, created_at, updated_at)
                     VALUES (?1, 2, ?2, ?3, 'WhatsApp Group', ?4, ?4)",
                    rusqlite::params![conversation_id, thread_id, kind, now],
                )
                .unwrap();
            }

            fn insert_whatsapp_local_participant(
                &self,
                conversation_id: i64,
                external_id: &str,
                display_name: &str,
            ) {
                let conn = self.state.conn.lock().unwrap();
                let now = "2026-08-10T00:00:00Z";
                let identity_id = conn
                    .query_row(
                        "INSERT INTO channel_identities
                         (channel, channel_account_id, identity_namespace, external_id, display_name, created_at, updated_at)
                         VALUES ('whatsapp', 2, 'phone', ?1, ?2, ?3, ?3)
                         RETURNING id",
                        rusqlite::params![external_id, display_name, now],
                        |row| row.get::<_, i64>(0),
                    )
                    .unwrap();
                conn.execute(
                    "INSERT INTO conversation_participants
                     (conversation_id, identity_id, participant_role, joined_at)
                     VALUES (?1, ?2, 'member', ?3)",
                    rusqlite::params![conversation_id, identity_id, now],
                )
                .unwrap();
            }

            fn insert_local_reaction_actor(&self, conversation_id: i64, external_id: &str) {
                let conn = self.state.conn.lock().unwrap();
                let message_id = conn
                    .query_row(
                        "INSERT INTO messages
                         (conversation_id, direction, content_type, content_json, ingested_at,
                          status)
                         VALUES (?1, 'inbound', 'text', '{}', '2026-08-10T00:00:00Z', 'received')
                         RETURNING id",
                        [conversation_id],
                        |row| row.get::<_, i64>(0),
                    )
                    .unwrap();
                conn.execute(
                    "INSERT INTO message_reactions
                     (message_id, actor_external_id, reaction, count, reacted_at)
                     VALUES (?1, ?2, 'heart', 1, '2026-08-10T00:00:00Z')",
                    rusqlite::params![message_id, external_id],
                )
                .unwrap();
            }

            fn identity_count(&self, external_id: &str) -> i64 {
                self.state
                    .conn
                    .lock()
                    .unwrap()
                    .query_row(
                        "SELECT COUNT(*) FROM channel_identities WHERE external_id = ?1",
                        [external_id],
                        |row| row.get(0),
                    )
                    .unwrap()
            }

            fn identity_namespace(&self, external_id: &str) -> String {
                self.state
                    .conn
                    .lock()
                    .unwrap()
                    .query_row(
                        "SELECT identity_namespace FROM channel_identities WHERE external_id = ?1",
                        [external_id],
                        |row| row.get(0),
                    )
                    .unwrap()
            }
        }

        impl Drop for GroupScanHarness {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.db_path);
                let _ = std::fs::remove_dir_all(&self.root);
            }
        }

        struct FakeGroupScanClient {
            event_tx: broadcast::Sender<ListenerEvent>,
            snapshot: Mutex<Result<Option<NormalizedGroupSnapshot>, String>>,
            board: Mutex<Result<GroupBoardUidReport, String>>,
            profiles: Mutex<Result<Vec<NormalizedProfile>, String>>,
            link_members: Mutex<Option<Vec<NormalizedProfile>>>,
            direct_history_requests: AtomicUsize,
        }

        impl FakeGroupScanClient {
            fn new(
                snapshot: Result<Option<NormalizedGroupSnapshot>, String>,
                board: Result<GroupBoardUidReport, String>,
                profiles: Result<Vec<NormalizedProfile>, String>,
            ) -> Self {
                let (event_tx, _) = broadcast::channel(4);
                Self {
                    event_tx,
                    snapshot: Mutex::new(snapshot),
                    board: Mutex::new(board),
                    profiles: Mutex::new(profiles),
                    link_members: Mutex::new(None),
                    direct_history_requests: AtomicUsize::new(0),
                }
            }

            fn with_link_members(mut self, link_members: Vec<NormalizedProfile>) -> Self {
                self.link_members = Mutex::new(Some(link_members));
                self
            }
        }

        #[async_trait]
        impl ZaloClient for FakeGroupScanClient {
            fn subscribe(&self) -> broadcast::Receiver<ListenerEvent> {
                self.event_tx.subscribe()
            }

            fn request_old_messages(
                &self,
                _thread_type: ThreadType,
                _cursor: Option<&str>,
            ) -> anyhow::Result<()> {
                self.direct_history_requests.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }

            fn request_old_reactions(
                &self,
                _thread_type: ThreadType,
                _cursor: Option<&str>,
            ) -> anyhow::Result<()> {
                Ok(())
            }

            async fn get_all_group_snapshots(
                &self,
            ) -> anyhow::Result<Vec<NormalizedGroupSnapshot>> {
                Ok(Vec::new())
            }

            async fn get_group_snapshot(
                &self,
                _group_id: &str,
            ) -> anyhow::Result<Option<NormalizedGroupSnapshot>> {
                self.snapshot
                    .lock()
                    .unwrap()
                    .clone()
                    .map_err(anyhow::Error::msg)
            }

            async fn send_text(
                &self,
                _thread_id: &str,
                _thread_type: ThreadType,
                _text: &str,
            ) -> anyhow::Result<String> {
                Err(anyhow::anyhow!("send_text must not run in Deep Scan tests"))
            }

            async fn fetch_group_member_profiles(
                &self,
                _user_ids: &[String],
            ) -> anyhow::Result<Vec<NormalizedProfile>> {
                self.profiles
                    .lock()
                    .unwrap()
                    .clone()
                    .map_err(anyhow::Error::msg)
            }

            async fn fetch_group_board_uid_report(
                &self,
                _group_id: &str,
            ) -> anyhow::Result<GroupBoardUidReport> {
                self.board
                    .lock()
                    .unwrap()
                    .clone()
                    .map_err(anyhow::Error::msg)
            }

            async fn fetch_group_link_detail(
                &self,
                _group_id: &str,
            ) -> anyhow::Result<Option<String>> {
                if self.link_members.lock().unwrap().is_some() {
                    Ok(Some("https://zalo.me/g/testlink".to_string()))
                } else {
                    Ok(None)
                }
            }

            async fn fetch_group_link_members(
                &self,
                _link: &str,
                page: u32,
            ) -> anyhow::Result<Option<(Vec<NormalizedProfile>, bool)>> {
                if page == 1 {
                    if let Some(mems) = self.link_members.lock().unwrap().clone() {
                        return Ok(Some((mems, false)));
                    }
                }
                Ok(None)
            }

            fn stop(&self) {}
        }

        struct FakeGroupScanRuntime {
            client: Option<Arc<FakeGroupScanClient>>,
            whatsapp_client: Option<Arc<dyn crate::conversation::whatsapp::client::WhatsAppClient>>,
            backfill: GroupMessageBackfillReport,
            connected_calls: AtomicUsize,
            backfill_calls: AtomicUsize,
        }

        impl FakeGroupScanRuntime {
            fn new(
                client: Option<Arc<FakeGroupScanClient>>,
                outcome: GroupMessageBackfillOutcome,
            ) -> Self {
                let warning = (outcome != GroupMessageBackfillOutcome::Completed)
                    .then(|| format!("backfill outcome: {outcome:?}"));
                Self {
                    client,
                    whatsapp_client: None,
                    backfill: GroupMessageBackfillReport {
                        provider_account_key: "account-a".to_string(),
                        scope: "account_group_messages".to_string(),
                        outcome,
                        joined_existing_run: false,
                        pages_requested: 2,
                        pages_received: 1,
                        start_cursor: Some("cursor-start".to_string()),
                        durable_cursor: Some("cursor-durable".to_string()),
                        stop_reason: Some("test_stop".to_string()),
                        warning,
                    },
                    connected_calls: AtomicUsize::new(0),
                    backfill_calls: AtomicUsize::new(0),
                }
            }

            fn with_whatsapp_client(
                mut self,
                whatsapp_client: Arc<dyn crate::conversation::whatsapp::client::WhatsAppClient>,
            ) -> Self {
                self.whatsapp_client = Some(whatsapp_client);
                self
            }
        }

        #[async_trait]
        impl commands::GroupMemberScanRuntime for FakeGroupScanRuntime {
            async fn connected_client(
                &self,
                _provider_account_key: &str,
            ) -> Option<Arc<dyn ZaloClient>> {
                self.connected_calls.fetch_add(1, Ordering::SeqCst);
                self.client
                    .as_ref()
                    .map(|client| client.clone() as Arc<dyn ZaloClient>)
            }

            async fn backfill_group_messages(
                &self,
                _provider_account_key: &str,
                _timeout: std::time::Duration,
            ) -> GroupMessageBackfillReport {
                self.backfill_calls.fetch_add(1, Ordering::SeqCst);
                self.backfill.clone()
            }

            async fn connected_whatsapp_client(
                &self,
                _account_id: i64,
            ) -> Option<Arc<dyn crate::conversation::whatsapp::client::WhatsAppClient>>
            {
                self.whatsapp_client.clone()
            }
        }

        fn snapshot(membership_complete: bool) -> NormalizedGroupSnapshot {
            NormalizedGroupSnapshot {
                provider_account_key: "account-a".to_string(),
                external_thread_id: "group-a".to_string(),
                title: "Group A".to_string(),
                avatar_url: None,
                participants: vec![NormalizedGroupParticipant {
                    identity: NormalizedIdentity {
                        identity_namespace: "zalo_uid".to_string(),
                        external_id: "snapshot-uid".to_string(),
                        display_name: Some("Snapshot Member".to_string()),
                        avatar_url: None,
                        lead_id: None,
                    },
                    participant_role: "member".to_string(),
                }],
                membership_complete,
                occurred_at: "2026-08-10T00:00:00Z".to_string(),
            }
        }

        #[tokio::test]
        async fn non_group_is_rejected_before_provider_work() {
            let harness = GroupScanHarness::new();
            harness.insert_conversation(100, "direct");
            let runtime = FakeGroupScanRuntime::new(None, GroupMessageBackfillOutcome::Unavailable);

            let result = commands::sync_and_deep_scan_group_members_impl(
                &harness.state,
                harness.service.as_ref(),
                &runtime,
                100,
            )
            .await;

            assert!(result.is_err());
            assert_eq!(runtime.connected_calls.load(Ordering::SeqCst), 0);
            assert_eq!(runtime.backfill_calls.load(Ordering::SeqCst), 0);
        }

        #[tokio::test]
        async fn disconnected_scan_returns_local_evidence_with_unavailable_diagnostics() {
            let harness = GroupScanHarness::new();
            harness.insert_conversation(101, "group");
            harness.insert_local_participant(101, "local-uid");
            let runtime = FakeGroupScanRuntime::new(None, GroupMessageBackfillOutcome::Unavailable);

            let result = commands::sync_and_deep_scan_group_members_impl(
                &harness.state,
                harness.service.as_ref(),
                &runtime,
                101,
            )
            .await
            .unwrap();

            assert!(result
                .members
                .iter()
                .any(|member| member.external_id.as_deref() == Some("local-uid")));
            assert!(!result.diagnostics.direct_snapshot.attempted);
            assert_eq!(
                result.diagnostics.history_backfill.outcome,
                GroupMessageBackfillOutcome::Unavailable
            );
            assert!(result.diagnostics.partial_result);
            assert_eq!(runtime.backfill_calls.load(Ordering::SeqCst), 1);
        }

        #[tokio::test]
        async fn disconnected_scan_canonicalizes_raw_reaction_actor_evidence() {
            let harness = GroupScanHarness::new();
            harness.insert_conversation(104, "group");
            harness.insert_local_reaction_actor(104, "reaction-only-uid");
            let runtime = FakeGroupScanRuntime::new(None, GroupMessageBackfillOutcome::Unavailable);

            let result = commands::sync_and_deep_scan_group_members_impl(
                &harness.state,
                harness.service.as_ref(),
                &runtime,
                104,
            )
            .await
            .unwrap();

            assert!(result
                .members
                .iter()
                .any(|member| member.external_id.as_deref() == Some("reaction-only-uid")));
            assert_eq!(result.diagnostics.local_evidence.reaction_uid_count, 1);
            assert_eq!(result.diagnostics.local_evidence.committed_uid_count, 1);
            assert_eq!(harness.identity_namespace("reaction-only-uid"), "zalo_uid");
        }

        #[tokio::test]
        async fn timeout_and_board_partial_failure_are_structured_without_direct_command_511() {
            let harness = GroupScanHarness::new();
            harness.insert_conversation(102, "group");
            let client = Arc::new(FakeGroupScanClient::new(
                Ok(Some(snapshot(false))),
                Ok(GroupBoardUidReport {
                    uids: vec![
                        "board-uid-a".to_string(),
                        "board-uid-a".to_string(),
                        "board-uid-b".to_string(),
                    ],
                    pages_attempted: 3,
                    pages_succeeded: 2,
                    items_inspected: 7,
                    polls_attempted: 2,
                    polls_succeeded: 1,
                    stop_reason: Some("board_page_failed".to_string()),
                    warnings: vec![GroupBoardWarning {
                        source: "poll".to_string(),
                        code: "poll_detail_failed".to_string(),
                        context: Some("poll_id=9".to_string()),
                        message: "poll unavailable".to_string(),
                    }],
                }),
                Ok(vec![NormalizedProfile {
                    external_id: "board-uid-a".to_string(),
                    display_name: Some("Board A".to_string()),
                    avatar_url: Some("https://img/a.png".to_string()),
                }]),
            ));
            let runtime = FakeGroupScanRuntime::new(
                Some(client.clone()),
                GroupMessageBackfillOutcome::TimedOut,
            );

            let result = commands::sync_and_deep_scan_group_members_impl(
                &harness.state,
                harness.service.as_ref(),
                &runtime,
                102,
            )
            .await
            .unwrap();

            assert_eq!(runtime.backfill_calls.load(Ordering::SeqCst), 1);
            assert_eq!(client.direct_history_requests.load(Ordering::SeqCst), 0);
            assert_eq!(
                result.diagnostics.history_backfill.outcome,
                GroupMessageBackfillOutcome::TimedOut
            );
            assert_eq!(result.diagnostics.board.pages_attempted, 3);
            assert_eq!(result.diagnostics.board.pages_succeeded, 2);
            assert_eq!(result.diagnostics.board.committed_uid_count, 2);
            assert_eq!(result.diagnostics.board.warnings.len(), 1);
            assert!(result
                .diagnostics
                .warnings
                .iter()
                .any(|warning| warning.code == "poll_detail_failed"));
            assert!(result
                .members
                .iter()
                .any(|member| member.external_id.as_deref() == Some("board-uid-b")));
            assert_eq!(harness.identity_namespace("board-uid-a"), "zalo_uid");
        }

        #[tokio::test]
        async fn board_top_level_failure_marks_scan_as_partial() {
            let harness = GroupScanHarness::new();
            harness.insert_conversation(105, "group");
            let client = Arc::new(FakeGroupScanClient::new(
                Ok(Some(snapshot(true))),
                Err("board unavailable".to_string()),
                Ok(vec![NormalizedProfile {
                    external_id: "snapshot-uid".to_string(),
                    display_name: Some("Snapshot Member".to_string()),
                    avatar_url: None,
                }]),
            ));
            let runtime =
                FakeGroupScanRuntime::new(Some(client), GroupMessageBackfillOutcome::Completed);

            let result = commands::sync_and_deep_scan_group_members_impl(
                &harness.state,
                harness.service.as_ref(),
                &runtime,
                105,
            )
            .await
            .unwrap();

            assert!(result.diagnostics.direct_snapshot.complete);
            assert_eq!(result.diagnostics.profile_hydration.failed, 0);
            assert!(result.diagnostics.partial_result);
            assert!(result
                .diagnostics
                .warnings
                .iter()
                .any(|warning| warning.code == "board_scan_failed"));
        }

        #[tokio::test]
        async fn mismatched_direct_snapshot_is_rejected_without_persistence() {
            let harness = GroupScanHarness::new();
            harness.insert_conversation(106, "group");
            let mut mismatched_snapshot = snapshot(true);
            mismatched_snapshot.provider_account_key = "account-b".to_string();
            mismatched_snapshot.external_thread_id = "group-b".to_string();
            let client = Arc::new(FakeGroupScanClient::new(
                Ok(Some(mismatched_snapshot)),
                Ok(GroupBoardUidReport {
                    uids: Vec::new(),
                    pages_attempted: 1,
                    pages_succeeded: 1,
                    items_inspected: 0,
                    polls_attempted: 0,
                    polls_succeeded: 0,
                    stop_reason: Some("short_page".to_string()),
                    warnings: Vec::new(),
                }),
                Ok(Vec::new()),
            ));
            let runtime =
                FakeGroupScanRuntime::new(Some(client), GroupMessageBackfillOutcome::Completed);

            let result = commands::sync_and_deep_scan_group_members_impl(
                &harness.state,
                harness.service.as_ref(),
                &runtime,
                106,
            )
            .await
            .unwrap();

            assert!(!result.diagnostics.direct_snapshot.complete);
            assert_eq!(result.diagnostics.direct_snapshot.member_count, 0);
            assert!(result.diagnostics.partial_result);
            assert!(result
                .diagnostics
                .warnings
                .iter()
                .any(|warning| warning.code == "snapshot_mismatch"));
            assert_eq!(harness.identity_count("snapshot-uid"), 0);
        }

        #[tokio::test]
        async fn board_persistence_failure_is_fatal_and_rolls_back_full_batch() {
            let harness = GroupScanHarness::new();
            harness.insert_conversation(103, "group");
            let client = Arc::new(FakeGroupScanClient::new(
                Ok(None),
                Ok(GroupBoardUidReport {
                    uids: vec!["valid-before-error".to_string(), "0".to_string()],
                    pages_attempted: 1,
                    pages_succeeded: 1,
                    items_inspected: 1,
                    polls_attempted: 0,
                    polls_succeeded: 0,
                    stop_reason: Some("short_page".to_string()),
                    warnings: Vec::new(),
                }),
                Ok(Vec::new()),
            ));
            let runtime =
                FakeGroupScanRuntime::new(Some(client), GroupMessageBackfillOutcome::Completed);

            let result = commands::sync_and_deep_scan_group_members_impl(
                &harness.state,
                harness.service.as_ref(),
                &runtime,
                103,
            )
            .await;

            assert!(result.is_err());
            assert_eq!(harness.identity_count("valid-before-error"), 0);
        }

        #[tokio::test]
        async fn group_link_members_are_committed_during_deep_scan() {
            let harness = GroupScanHarness::new();
            harness.insert_conversation(105, "group");
            let link_profile = NormalizedProfile {
                external_id: "link-member-uid".to_string(),
                display_name: Some("Link Member".to_string()),
                avatar_url: None,
            };
            let client = Arc::new(
                FakeGroupScanClient::new(
                    Ok(None),
                    Ok(GroupBoardUidReport::default()),
                    Ok(vec![link_profile.clone()]),
                )
                .with_link_members(vec![link_profile]),
            );
            let runtime =
                FakeGroupScanRuntime::new(Some(client), GroupMessageBackfillOutcome::Completed);

            let result = commands::sync_and_deep_scan_group_members_impl(
                &harness.state,
                harness.service.as_ref(),
                &runtime,
                105,
            )
            .await
            .unwrap();

            assert!(result
                .members
                .iter()
                .any(|member| member.external_id.as_deref() == Some("link-member-uid")));
            assert!(result.diagnostics.group_link.attempted);
            assert!(result.diagnostics.group_link.link_retrieved);
            assert_eq!(result.diagnostics.group_link.committed_uid_count, 1);
            assert_eq!(harness.identity_count("link-member-uid"), 1);
        }

        #[tokio::test]
        async fn whatsapp_group_scan_with_connected_client_syncs_metadata_and_returns_complete_membership(
        ) {
            let harness = GroupScanHarness::new();
            let thread_id = "120363000000000001@g.us";
            harness.insert_whatsapp_conversation(501, "group", thread_id);

            let wa_client =
                Arc::new(crate::conversation::whatsapp::client::MockWhatsAppClient::new());
            let group_jid: whatsapp_rust::Jid = thread_id.parse().unwrap();
            let p1_jid: whatsapp_rust::Jid = "84901111111@s.whatsapp.net".parse().unwrap();
            let p2_jid: whatsapp_rust::Jid = "84902222222@s.whatsapp.net".parse().unwrap();

            let group_meta = whatsapp_rust::GroupMetadata {
                id: group_jid,
                subject: "WhatsApp Test Group".to_string(),
                participants: vec![
                    whatsapp_rust::GroupParticipant {
                        jid: p1_jid.clone(),
                        phone_number: Some(p1_jid.clone()),
                        lid: None,
                        username: None,
                        participant_type: whatsapp_rust::ParticipantType::Admin,
                        details: None,
                    },
                    whatsapp_rust::GroupParticipant {
                        jid: p2_jid.clone(),
                        phone_number: Some(p2_jid.clone()),
                        lid: None,
                        username: None,
                        participant_type: whatsapp_rust::ParticipantType::Member,
                        details: None,
                    },
                ],
                ..Default::default()
            };
            wa_client.set_group_info(thread_id, Ok(group_meta));

            let runtime = FakeGroupScanRuntime::new(None, GroupMessageBackfillOutcome::Completed)
                .with_whatsapp_client(wa_client);

            let result = commands::sync_and_deep_scan_group_members_impl(
                &harness.state,
                harness.service.as_ref(),
                &runtime,
                501,
            )
            .await
            .unwrap();

            assert_eq!(result.discovered_count, 2);
            assert!(result.membership_complete);
            assert!(result.diagnostics.direct_snapshot.attempted);
            assert!(result.diagnostics.direct_snapshot.complete);
            assert_eq!(result.diagnostics.direct_snapshot.member_count, 2);
            assert!(!result.diagnostics.partial_result);
            assert!(result.warning.is_none());

            let members = result.members;
            assert_eq!(members.len(), 2);
            let admin = members
                .iter()
                .find(|m| m.external_id.as_deref() == Some("84901111111"))
                .unwrap();
            assert_eq!(admin.participant_role, "admin");
            let member = members
                .iter()
                .find(|m| m.external_id.as_deref() == Some("84902222222"))
                .unwrap();
            assert_eq!(member.participant_role, "member");
        }

        #[tokio::test]
        async fn whatsapp_group_scan_with_disconnected_client_returns_local_evidence_and_warning() {
            let harness = GroupScanHarness::new();
            let thread_id = "120363000000000002@g.us";
            harness.insert_whatsapp_conversation(502, "group", thread_id);
            harness.insert_whatsapp_local_participant(502, "84903333333", "Offline Member");

            let runtime = FakeGroupScanRuntime::new(None, GroupMessageBackfillOutcome::Unavailable);

            let result = commands::sync_and_deep_scan_group_members_impl(
                &harness.state,
                harness.service.as_ref(),
                &runtime,
                502,
            )
            .await
            .unwrap();

            assert_eq!(result.discovered_count, 1);
            assert!(!result.membership_complete);
            assert!(!result.diagnostics.direct_snapshot.attempted);
            assert!(!result.diagnostics.direct_snapshot.complete);
            assert!(result.diagnostics.partial_result);
            assert_eq!(result.stop_reason.as_deref(), Some("client_offline"));
            assert!(result.warning.is_some());
            assert_eq!(result.diagnostics.warnings[0].code, "client_offline");
            assert_eq!(
                result.members[0].external_id.as_deref(),
                Some("84903333333")
            );
        }

        #[tokio::test]
        async fn whatsapp_group_scan_query_failure_returns_partial_result_and_warning() {
            let harness = GroupScanHarness::new();
            let thread_id = "120363000000000003@g.us";
            harness.insert_whatsapp_conversation(503, "group", thread_id);
            harness.insert_whatsapp_local_participant(503, "84904444444", "Fallback Member");

            let wa_client =
                Arc::new(crate::conversation::whatsapp::client::MockWhatsAppClient::new());
            wa_client.set_next_group_info_result(Err(anyhow::anyhow!("Network timeout")));

            let runtime = FakeGroupScanRuntime::new(None, GroupMessageBackfillOutcome::Completed)
                .with_whatsapp_client(wa_client);

            let result = commands::sync_and_deep_scan_group_members_impl(
                &harness.state,
                harness.service.as_ref(),
                &runtime,
                503,
            )
            .await
            .unwrap();

            assert_eq!(result.discovered_count, 1);
            assert!(!result.membership_complete);
            assert!(result.diagnostics.direct_snapshot.attempted);
            assert!(!result.diagnostics.direct_snapshot.complete);
            assert!(result.diagnostics.partial_result);
            assert_eq!(result.stop_reason.as_deref(), Some("whatsapp_query_failed"));
            assert!(result.warning.is_some());
            assert_eq!(result.diagnostics.warnings[0].code, "whatsapp_query_failed");
            assert_eq!(
                result.members[0].external_id.as_deref(),
                Some("84904444444")
            );
        }
    }

    #[test]
    fn group_member_export_includes_quote_owner_evidence() {
        let conn = db::init_db(":memory:").unwrap();
        let now = "2026-08-10T00:00:00Z";
        conn.execute(
            "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, created_at, updated_at)
             VALUES (1, 'zalo', 'key1', 'Zalo Acc 1', ?1, ?1)",
            [now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, title, created_at, updated_at)
             VALUES (200, 1, 'g200', 'group', 'Test Group 2', ?1, ?1)",
            [now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO channel_identities
             (id, channel, identity_namespace, external_id, display_name, created_at, updated_at)
             VALUES (101, 'zalo', 'zalo_uid', 'quote-owner-uid', 'Quote Owner', ?1, ?1)",
            [now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO messages
             (conversation_id, direction, content_type, content_json, ingested_at, status,
              reply_sender_identity_id)
             VALUES (200, 'inbound', 'text', '{}', ?1, 'received', 101)",
            [now],
        )
        .unwrap();

        let members = commands::fetch_group_members_impl(&conn, 200).unwrap();

        assert_eq!(members.len(), 1);
        assert_eq!(members[0].external_id.as_deref(), Some("quote-owner-uid"));
    }

    #[test]
    fn test_whatsapp_group_member_fetching_and_batch_import() {
        let root = tempfile::tempdir().unwrap();
        let db_path = root.path().join("test_whatsapp_group.db");
        let conn = db::init_db(db_path.to_str().unwrap()).unwrap();
        let now = "2026-08-27T00:00:00Z";

        conn.execute(
            "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, created_at, updated_at)
             VALUES (2, 'whatsapp', '84901234567', 'WA Account', ?1, ?1)",
            [now],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO channel_conversations
             (id, channel_account_id, external_thread_id, kind, title, created_at, updated_at)
             VALUES (300, 2, '123456789-987654@g.us', 'group', 'AI Builders Community', ?1, ?1)",
            [now],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO channel_identities
             (id, channel, channel_account_id, identity_namespace, external_id, display_name, created_at, updated_at)
             VALUES
             (3001, 'whatsapp', 2, 'phone', '8613926842913', 'Vject Wang', ?1, ?1),
             (3002, 'whatsapp', 2, 'phone', '84909888777', 'Tymon', ?1, ?1)",
            [now],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO conversation_participants
             (conversation_id, identity_id, participant_role, joined_at)
             VALUES
             (300, 3001, 'admin', ?1),
             (300, 3002, 'member', ?1)",
            [now],
        )
        .unwrap();

        let members = commands::fetch_group_members_impl(&conn, 300).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].display_name, "Vject Wang");
        assert_eq!(members[0].participant_role, "admin");
        assert_eq!(members[1].display_name, "Tymon");
        assert_eq!(members[1].participant_role, "member");
        drop(conn);

        let repository = std::sync::Arc::new(
            crate::conversation::repository::ConversationRepository::new(
                db_path.to_string_lossy().to_string(),
            ),
        );
        let events =
            std::sync::Arc::new(crate::conversation::events::ConversationEventPublisher::new(8));
        let service = crate::conversation::service::ConversationService::new(
            repository,
            events.clone(),
            root.path().join("spool"),
        );

        let result = commands::batch_import_group_members_to_leads_impl(
            &service,
            commands::BatchImportGroupMembersRequest {
                conversation_id: 300,
                members: vec![commands::ImportMemberItem {
                    identity_id: Some(3001),
                    external_id: Some("8613926842913".to_string()),
                    display_name: "Vject Wang".to_string(),
                    avatar_url: None,
                    phone: Some("8613926842913".to_string()),
                }],
                category_ids: Vec::new(),
                new_category_name: Some("WhatsApp Community".to_string()),
                group_title: "AI Builders Community".to_string(),
                skip_existing: Some(true),
            },
        )
        .unwrap();

        assert_eq!(result.imported_count, 1);
        assert_eq!(result.assigned_category_ids.len(), 1);

        let conn = crate::db::open_db_connection(db_path.to_str().unwrap()).unwrap();
        let updated_members = commands::fetch_group_members_impl(&conn, 300).unwrap();
        assert_eq!(updated_members.len(), 2);
        assert_eq!(updated_members[0].display_name, "Vject Wang");
        assert!(updated_members[0].lead_id.is_some());
        assert_eq!(updated_members[0].categories.len(), 1);
        assert_eq!(updated_members[0].categories[0].name, "WhatsApp Community");
    }

    #[test]
    fn test_get_hardware_id_returns_formatted_hwid() {
        let hw_info = commands::get_hardware_id().unwrap();
        assert!(hw_info.hardware_id.starts_with("HWID-"));
        assert_eq!(hw_info.hardware_id.len(), 24); // HWID-XXXX-XXXX-XXXX-XXXX = 24 chars
        assert!(!hw_info.hostname.is_empty());
        assert!(!hw_info.os_info.is_empty());
    }

    #[tokio::test]
    async fn test_export_leads_csv_file_validation_and_writing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let valid_csv_path = temp_dir.path().join("leads.csv");
        let content = "Name,Company,Phone\nJohn,Acme,0901234567";

        // 1. Valid .csv write succeeds
        let res = commands::write_leads_csv_to_path(&valid_csv_path, content).await;
        assert!(res.is_ok());
        let written_target = res.unwrap();
        let read_back = std::fs::read_to_string(&written_target).unwrap();
        assert_eq!(read_back, content);

        // 2. Non-.csv extension is rejected
        let invalid_ext_path = temp_dir.path().join("script.sh");
        let res_invalid = commands::write_leads_csv_to_path(&invalid_ext_path, "echo hack").await;
        assert!(res_invalid.is_err());
        assert!(res_invalid.unwrap_err().contains("Only .csv files"));

        // 3. Non-existent parent directory is rejected
        let non_existent_dir_path = temp_dir
            .path()
            .join("non_existent_folder_xyz")
            .join("leads.csv");
        let res_no_parent =
            commands::write_leads_csv_to_path(&non_existent_dir_path, content).await;
        assert!(res_no_parent.is_err());
        assert!(res_no_parent
            .unwrap_err()
            .contains("Destination directory does not exist"));

        // 4. Direct symlinks to another file are rejected
        let target_file = temp_dir.path().join("real_target.csv");
        std::fs::write(&target_file, "real").unwrap();
        #[cfg(unix)]
        {
            let symlink_path = temp_dir.path().join("symlink.csv");
            std::os::unix::fs::symlink(&target_file, &symlink_path).unwrap();
            let res_symlink = commands::write_leads_csv_to_path(&symlink_path, "new content").await;
            assert!(res_symlink.is_err());
            assert!(res_symlink
                .unwrap_err()
                .contains("Export to symbolic links is not permitted"));
            // Ensure target file was NOT overwritten
            assert_eq!(std::fs::read_to_string(&target_file).unwrap(), "real");
        }

        // 5. Parent directory symlink pinning & TOCTOU protection
        #[cfg(unix)]
        {
            let real_parent = temp_dir.path().join("real_parent_dir");
            std::fs::create_dir(&real_parent).unwrap();

            // When path points to a file in an existing real directory, write succeeds
            let normal_file = real_parent.join("exported.csv");
            let res_normal = commands::write_leads_csv_to_path(&normal_file, content).await;
            assert!(res_normal.is_ok());

            // If an attacker attempts to point to a target that is a directory
            let dir_as_file = real_parent.join("dir_target.csv");
            std::fs::create_dir(&dir_as_file).unwrap();
            let res_dir = commands::write_leads_csv_to_path(&dir_as_file, content).await;
            assert!(res_dir.is_err());
        }
    }

    #[tokio::test]
    async fn test_export_leads_csv_file_with_dialog_workflow() {
        let temp_dir = tempfile::tempdir().unwrap();
        let valid_path = temp_dir.path().join("dialog_export.csv");
        let content = "Name,Phone\nTest,0901234567";

        // 1. Dialog returns valid path -> writes and returns Some(path)
        let valid_path_clone = valid_path.clone();
        let res_ok =
            commands::export_leads_csv_file_with_dialog("default.csv", content, move || {
                Ok(Some(valid_path_clone))
            })
            .await;
        assert!(res_ok.is_ok());
        let returned_path = res_ok.unwrap();
        assert!(returned_path.is_some());
        assert!(returned_path.unwrap().ends_with("dialog_export.csv"));
        assert_eq!(std::fs::read_to_string(&valid_path).unwrap(), content);

        // 2. Dialog cancelled by user -> returns Ok(None) without writing
        let unwritten_path = temp_dir.path().join("cancelled.csv");
        let res_cancel =
            commands::export_leads_csv_file_with_dialog("default.csv", content, || Ok(None)).await;
        assert!(res_cancel.is_ok());
        assert!(res_cancel.unwrap().is_none());
        assert!(!unwritten_path.exists());

        // 3. Dialog backend error -> returns Err
        let res_err = commands::export_leads_csv_file_with_dialog("default.csv", content, || {
            Err("Native dialog failed to open".to_string())
        })
        .await;
        assert!(res_err.is_err());
        assert_eq!(res_err.unwrap_err(), "Native dialog failed to open");

        // 4. Dialog worker thread panic -> caught and returns Err
        let res_panic = commands::export_leads_csv_file_with_dialog("default.csv", content, || {
            panic!("Worker crash")
        })
        .await;
        assert!(res_panic.is_err());
        assert!(res_panic.unwrap_err().contains("Dialog worker failed"));
    }

    #[tokio::test]
    async fn test_export_leads_csv_file_symlink_and_toctou_adversarial_matrix() {
        #[cfg(unix)]
        {
            let temp_dir = tempfile::tempdir().unwrap();
            let content = "Name,Phone\nVictim,0901234567";
            let sensitive_file = temp_dir.path().join("sensitive.csv");
            std::fs::write(&sensitive_file, "DO_NOT_OVERWRITE").unwrap();

            let attacker_symlink = temp_dir.path().join("attacker_link.csv");
            std::os::unix::fs::symlink(&sensitive_file, &attacker_symlink).unwrap();

            let res = commands::write_leads_csv_to_path(&attacker_symlink, content).await;
            assert!(res.is_err());
            assert!(res.unwrap_err().contains("symbolic links"));
            assert_eq!(
                std::fs::read_to_string(&sensitive_file).unwrap(),
                "DO_NOT_OVERWRITE"
            );

            // 2. Ancestor directory replaced with symlink is rejected at descriptor open
            let target_dir = temp_dir.path().join("swapped_dir");
            std::fs::create_dir(&target_dir).unwrap();

            let sensitive_dest_dir = temp_dir.path().join("sensitive_dest_dir");
            std::fs::create_dir(&sensitive_dest_dir).unwrap();
            let sensitive_in_dir = sensitive_dest_dir.join("leaked.csv");
            std::fs::write(&sensitive_in_dir, "SENSITIVE").unwrap();

            // Simulate TOCTOU: remove directory and replace it with a symlink to sensitive_dest_dir
            std::fs::remove_dir(&target_dir).unwrap();
            std::os::unix::fs::symlink(&sensitive_dest_dir, &target_dir).unwrap();

            // Attempt to write through the symlinked ancestor directory
            let res =
                commands::write_leads_csv_to_path(&target_dir.join("leaked.csv"), content).await;
            assert!(res.is_err());
            let err_msg = res.unwrap_err();
            assert!(
                err_msg.contains("symbolic links")
                    || err_msg.contains("cannot contain symbolic links")
            );
            assert_eq!(
                std::fs::read_to_string(&sensitive_in_dir).unwrap(),
                "SENSITIVE"
            );

            // 3. FIFO / named pipe protection: opening existing FIFO never blocks and rejects non-regular file
            use std::os::unix::ffi::OsStrExt;
            let fifo_path = temp_dir.path().join("fifo_target.csv");
            let mut fifo_c = fifo_path.as_os_str().as_bytes().to_vec();
            fifo_c.push(0);

            let res_fifo_create =
                unsafe { libc::mkfifo(fifo_c.as_ptr() as *const libc::c_char, 0o666) };
            if res_fifo_create == 0 {
                // Must return error immediately without hanging the thread
                let res = commands::write_leads_csv_to_path(&fifo_path, content).await;
                assert!(res.is_err());
                let err_msg = res.unwrap_err();
                assert!(
                    err_msg.contains("regular file")
                        || err_msg.contains("open export file")
                        || err_msg.contains("cannot be accessed")
                );
            }

            // 4. Real concurrent TOCTOU race test: background thread actively swaps directory with symlink
            use std::sync::atomic::{AtomicBool, Ordering};
            use std::sync::Arc;

            let race_root = temp_dir.path().join("race_root");
            std::fs::create_dir(&race_root).unwrap();

            let real_dir = race_root.join("real_folder");
            std::fs::create_dir(&real_dir).unwrap();

            let victim_dir = race_root.join("victim_folder");
            std::fs::create_dir(&victim_dir).unwrap();
            let victim_file = victim_dir.join("victim.csv");
            std::fs::write(&victim_file, "VICTIM_ORIGINAL").unwrap();

            let volatile_target = race_root.join("volatile_parent");
            std::fs::create_dir(&volatile_target).unwrap();

            let stop_flag = Arc::new(AtomicBool::new(false));
            let stop_clone = stop_flag.clone();
            let volatile_path = volatile_target.clone();
            let victim_path = victim_dir.clone();

            // Swapper thread continuously switches volatile_parent between real dir and symlink to victim_dir
            let swapper_handle = std::thread::spawn(move || {
                let mut is_symlink = false;
                while !stop_clone.load(Ordering::Relaxed) {
                    if is_symlink {
                        let _ = std::fs::remove_file(&volatile_path);
                        let _ = std::fs::create_dir(&volatile_path);
                        is_symlink = false;
                    } else {
                        let _ = std::fs::remove_dir(&volatile_path);
                        let _ = std::os::unix::fs::symlink(&victim_path, &volatile_path);
                        is_symlink = true;
                    }
                    std::thread::yield_now();
                }
                // Cleanup to real dir so tempdir drop succeeds
                let _ = std::fs::remove_file(&volatile_path);
                let _ = std::fs::create_dir(&volatile_path);
            });

            // Perform concurrent write attempts in a loop
            for _ in 0..40 {
                let res = commands::write_leads_csv_to_path(
                    &volatile_target.join("victim.csv"),
                    "COMPROMISED_DATA",
                )
                .await;
                // If the operation succeeded, it MUST have written to the real dir, NEVER the victim file
                // If it failed, it caught the symlink/descriptor error
                if let Ok(written_path) = res {
                    assert!(!written_path.starts_with(&victim_dir));
                }
                // Invariant: Victim file must NEVER be corrupted by any race
                assert_eq!(
                    std::fs::read_to_string(&victim_file).unwrap(),
                    "VICTIM_ORIGINAL"
                );
            }

            stop_flag.store(true, Ordering::Relaxed);
            let _ = swapper_handle.join();
        }
    }

    #[tokio::test]
    async fn test_command_inspect_lead_import_file() {
        use std::io::Write;
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let csv_content =
            "Name,Phone,Email\nAlpha,0912345678,alpha@test.com\nBeta,0987654321,beta@test.com\n";
        file.write_all(csv_content.as_bytes()).unwrap();

        let path = file.path().with_extension("csv");
        std::fs::copy(file.path(), &path).unwrap();

        let res =
            commands::inspect_lead_import_file(path.to_string_lossy().to_string(), None).await;
        assert!(res.is_ok());
        let inspection = res.unwrap();
        assert_eq!(inspection.file_type, "csv");
        assert_eq!(inspection.headers, vec!["Name", "Phone", "Email"]);
        assert_eq!(inspection.sample_rows.len(), 2);
        assert_eq!(inspection.total_rows_estimate, 2);

        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn test_command_inspect_nonexistent_file() {
        let res =
            commands::inspect_lead_import_file("/non/existent/path/xyz.csv".to_string(), None)
                .await;
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Tệp không tồn tại"));
    }

    #[test]
    fn test_is_public_ip_adversarial_matrix() {
        use std::net::IpAddr;

        // Disallowed IPv4 addresses
        let private_v4 = [
            "127.0.0.1",
            "127.255.255.255",
            "10.0.0.1",
            "10.255.255.255",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.0.1",
            "192.168.255.255",
            "169.254.169.254", // AWS/cloud metadata
            "0.0.0.0",
            "255.255.255.255",
            "100.64.0.1", // CGNAT
            "192.0.2.1",  // Documentation
            "198.18.0.1", // Benchmark
            "224.0.0.1",  // Multicast
            "240.0.0.1",  // Reserved
        ];
        for ip_str in private_v4 {
            let ip: IpAddr = ip_str.parse().unwrap();
            assert!(
                !commands::is_public_ip(ip),
                "Expected {} to be blocked",
                ip_str
            );
        }

        // Allowed IPv4 addresses
        let public_v4 = ["8.8.8.8", "1.1.1.1", "142.250.190.46", "104.26.10.228"];
        for ip_str in public_v4 {
            let ip: IpAddr = ip_str.parse().unwrap();
            assert!(
                commands::is_public_ip(ip),
                "Expected {} to be allowed",
                ip_str
            );
        }

        // Disallowed IPv6 addresses
        let private_v6 = [
            "::1",
            "::",
            "fc00::1",
            "fd00::1",
            "fe80::1",
            "ff02::1",
            "::ffff:127.0.0.1",
            "::ffff:192.168.1.1",
        ];
        for ip_str in private_v6 {
            let ip: IpAddr = ip_str.parse().unwrap();
            assert!(
                !commands::is_public_ip(ip),
                "Expected {} to be blocked",
                ip_str
            );
        }

        // Allowed IPv6 addresses
        let public_v6 = ["2607:f8b0:4005:805::200e", "2606:4700:4700::1111"];
        for ip_str in public_v6 {
            let ip: IpAddr = ip_str.parse().unwrap();
            assert!(
                commands::is_public_ip(ip),
                "Expected {} to be allowed",
                ip_str
            );
        }
    }

    #[tokio::test]
    async fn test_download_image_http_blocks_ssrf_destinations() {
        // Direct private IPs and localhost are strictly blocked
        let res_local = commands::download_image_http("http://127.0.0.1:8080/image.png").await;
        assert!(res_local.is_err());
        let err_str = res_local.unwrap_err();
        assert!(err_str.contains("non-public") || err_str.contains("private"));

        let res_private = commands::download_image_http("http://10.10.10.10/admin.png").await;
        assert!(res_private.is_err());

        let res_meta = commands::download_image_http("http://169.254.169.254/meta-data").await;
        assert!(res_meta.is_err());

        let res_scheme = commands::download_image_http("ftp://example.com/pic.png").await;
        assert!(res_scheme.is_err());
    }

    #[test]
    fn test_detect_image_format_and_integrity() {
        // PNG
        let png_bytes = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 1];
        assert_eq!(commands::detect_image_format(&png_bytes).unwrap(), "png");

        // JPEG
        let jpeg_bytes = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F'];
        assert_eq!(commands::detect_image_format(&jpeg_bytes).unwrap(), "jpg");

        // GIF
        let gif_bytes = b"GIF89a\x01\x00\x01\x00\x80\x00\x00";
        assert_eq!(commands::detect_image_format(gif_bytes).unwrap(), "gif");

        // WebP
        let webp_bytes = b"RIFF\x1a\x00\x00\x00WEBPVP8 \x0e\x00\x00\x00";
        assert_eq!(commands::detect_image_format(webp_bytes).unwrap(), "webp");

        // SVG
        let svg_bytes =
            b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"></svg>";
        assert_eq!(commands::detect_image_format(svg_bytes).unwrap(), "svg");

        // XML SVG
        let xml_svg_bytes = b"<?xml version=\"1.0\" encoding=\"utf-8\"?><svg width=\"100\"></svg>";
        assert_eq!(commands::detect_image_format(xml_svg_bytes).unwrap(), "svg");

        // Non-image / Malicious executable or script
        let bash_bytes = b"#!/bin/bash\necho hello world";
        assert!(commands::detect_image_format(bash_bytes).is_err());

        let exe_bytes = b"MZ\x90\x00\x03\x00\x00\x00";
        assert!(commands::detect_image_format(exe_bytes).is_err());
    }

    #[tokio::test]
    async fn test_write_image_bytes_to_path_validation_and_writing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let valid_png_path = temp_dir.path().join("test_image.png");
        let png_bytes = [
            0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x01, 0x02,
        ];

        // 1. Valid write
        let res = commands::write_image_bytes_to_path(&valid_png_path, &png_bytes).await;
        assert!(res.is_ok());
        assert_eq!(std::fs::read(&valid_png_path).unwrap(), png_bytes);

        // 2. Reject non-image extension (e.g. .exe, .sh, .txt)
        let invalid_ext_path = temp_dir.path().join("malicious.sh");
        let err_res = commands::write_image_bytes_to_path(&invalid_ext_path, &png_bytes).await;
        assert!(err_res.is_err());
        assert!(err_res.unwrap_err().contains("valid image extension"));

        // 3. Reject path traversal
        let traversal_path = temp_dir.path().join("subdir/../escape.png");
        let trav_res = commands::write_image_bytes_to_path(&traversal_path, &png_bytes).await;
        assert!(trav_res.is_err());
        assert!(trav_res.unwrap_err().contains("Path traversal"));
    }

    #[tokio::test]
    async fn test_resolve_image_bytes_base64_and_data_url() {
        use base64::prelude::*;

        let raw_png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x01];
        let b64_str = BASE64_STANDARD.encode(raw_png);

        // 1. Direct base64 data
        let res_b64 = commands::resolve_image_bytes(None, Some(&b64_str)).await;
        assert!(res_b64.is_ok());
        assert_eq!(res_b64.unwrap(), raw_png);

        // 2. Data URL with prefix
        let data_url = format!("data:image/png;base64,{}", b64_str);
        let res_data_url = commands::resolve_image_bytes(Some(&data_url), None).await;
        assert!(res_data_url.is_ok());
        assert_eq!(res_data_url.unwrap(), raw_png);

        // 3. Empty inputs
        let res_empty = commands::resolve_image_bytes(None, None).await;
        assert!(res_empty.is_err());

        // 4. Invalid base64
        let res_invalid_b64 =
            commands::resolve_image_bytes(None, Some("!!!invalid base64!!!")).await;
        assert!(res_invalid_b64.is_err());

        // 5. Non-image decoded bytes rejection
        let invalid_image_b64 = BASE64_STANDARD.encode(b"hello world plaintext not an image");
        let res_non_image = commands::resolve_image_bytes(None, Some(&invalid_image_b64)).await;
        assert!(res_non_image.is_err());
        assert!(res_non_image
            .unwrap_err()
            .contains("supported image format"));
    }

    #[tokio::test]
    async fn test_save_image_file_with_dialog_workflow() {
        use base64::prelude::*;
        let temp_dir = tempfile::tempdir().unwrap();
        let valid_path = temp_dir.path().join("saved_photo.png");
        let png_bytes = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let b64 = BASE64_STANDARD.encode(png_bytes);

        // 1. User picks a valid path
        let valid_path_clone = valid_path.clone();
        let res_ok = commands::save_image_file_with_dialog(
            Some("suggested.png".to_string()),
            None,
            Some(b64.clone()),
            move |_| Ok(Some(valid_path_clone)),
        )
        .await;
        assert!(res_ok.is_ok());
        let returned = res_ok.unwrap();
        assert!(returned.is_some());
        assert!(returned.unwrap().ends_with("saved_photo.png"));
        assert_eq!(std::fs::read(&valid_path).unwrap(), png_bytes);

        // 2. User cancelled dialog (before any bytes fetched/processed)
        let res_cancel = commands::save_image_file_with_dialog(
            Some("suggested.png".to_string()),
            Some("http://127.0.0.1/never_called.png".to_string()),
            None,
            |_| Ok(None),
        )
        .await;
        assert!(res_cancel.is_ok());
        assert!(res_cancel.unwrap().is_none());

        // 3. User types name without extension -> rejected to enforce explicit extension
        let path_without_ext = temp_dir.path().join("photo_no_ext");
        let path_without_ext_clone = path_without_ext.clone();
        let res_no_ext = commands::save_image_file_with_dialog(
            Some("suggested".to_string()),
            None,
            Some(b64.clone()),
            move |_| Ok(Some(path_without_ext_clone)),
        )
        .await;
        assert!(res_no_ext.is_err());
        assert!(res_no_ext
            .unwrap_err()
            .contains("must include a valid image extension"));
        assert!(!path_without_ext.exists());

        // 4. Format alias acceptance: JPEG payload saved to .jpeg path succeeds
        let jpeg_bytes = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F'];
        let jpeg_b64 = BASE64_STANDARD.encode(jpeg_bytes);
        let jpeg_path = temp_dir.path().join("my_picture.jpeg");
        let jpeg_path_clone = jpeg_path.clone();
        let res_alias = commands::save_image_file_with_dialog(
            Some("my_picture.jpeg".to_string()),
            None,
            Some(jpeg_b64.clone()),
            move |_| Ok(Some(jpeg_path_clone)),
        )
        .await;
        assert!(res_alias.is_ok());
        assert!(jpeg_path.exists());
        assert_eq!(std::fs::read(&jpeg_path).unwrap(), jpeg_bytes);

        // 5. Format mismatch rejection: JPEG payload saved to .png path is rejected
        let mismatch_png_path = temp_dir.path().join("my_picture.png");
        let mismatch_png_clone = mismatch_png_path.clone();
        let res_mismatch = commands::save_image_file_with_dialog(
            Some("my_picture.png".to_string()),
            None,
            Some(jpeg_b64),
            move |_| Ok(Some(mismatch_png_clone)),
        )
        .await;
        assert!(res_mismatch.is_err());
        let err = res_mismatch.unwrap_err();
        assert!(err.contains("does not match detected image format"));
        assert!(!mismatch_png_path.exists());

        // 6. Overwrite isolation: Existing sentinel.jpg must remain untouched
        let existing_jpg = temp_dir.path().join("sentinel.jpg");
        std::fs::write(&existing_jpg, b"DO_NOT_OVERWRITE").unwrap();

        let req_png = temp_dir.path().join("other_file.png");
        let req_png_clone = req_png.clone();
        let res_png = commands::save_image_file_with_dialog(
            Some("other_file.png".to_string()),
            None,
            Some(b64),
            move |_| Ok(Some(req_png_clone)),
        )
        .await;
        assert!(res_png.is_ok());
        assert!(req_png.exists());
        assert_eq!(std::fs::read(&existing_jpg).unwrap(), b"DO_NOT_OVERWRITE");
    }

    #[tokio::test]
    async fn test_download_image_http_redirect_to_private_target_rejected() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        // Start mock HTTP server on random local port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Spawn mock server task that returns a 302 Redirect to an internal private IP
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf).await;
                let response = "HTTP/1.1 302 Found\r\nLocation: http://10.254.1.1:8080/private_image.png\r\n\r\n";
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });

        let target_url = format!("http://127.0.0.1:{}/start_download", addr.port());

        // Verifier allows initial mock server IP, but strictly checks subsequent hops
        let res = commands::download_image_http_test_helper(
            &target_url,
            |ip| {
                if ip == addr.ip() {
                    true // allow hop 1 in test harness
                } else {
                    commands::is_public_ip(ip) // evaluate hop 2 redirect destination
                }
            },
            None,
        )
        .await;

        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(err.contains("non-public") || err.contains("private"));
    }

    #[tokio::test]
    async fn test_download_image_http_redirect_chain_bounded() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Server continuously redirects to itself
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf).await;
                let response = format!(
                    "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{}/loop\r\n\r\n",
                    addr.port()
                );
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });

        let target_url = format!("http://127.0.0.1:{}/start", addr.port());
        let res = commands::download_image_http_test_helper(&target_url, |_| true, None).await;
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Too many redirects"));
    }

    #[tokio::test]
    async fn test_download_image_http_successful_redirect_and_no_proxy_isolation() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let png_bytes = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x01];

        // Server handles hop 1 (redirect) and hop 2 (image bytes)
        let png_bytes_clone = png_bytes;
        tokio::spawn(async move {
            // Hop 1: Redirect
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf).await;
                let response = format!(
                    "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{}/image.png\r\n\r\n",
                    addr.port()
                );
                let _ = stream.write_all(response.as_bytes()).await;
            }

            // Hop 2: Return PNG payload
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf).await;
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\n\r\n",
                    png_bytes_clone.len()
                );
                let _ = stream.write_all(header.as_bytes()).await;
                let _ = stream.write_all(&png_bytes_clone).await;
            }
        });

        // Set invalid proxy in environment to verify .no_proxy() connects directly to mock server
        std::env::set_var("HTTP_PROXY", "http://192.0.2.1:9999");
        std::env::set_var("http_proxy", "http://192.0.2.1:9999");

        let target_url = format!("http://127.0.0.1:{}/initial", addr.port());
        let res = commands::download_image_http_test_helper(&target_url, |_| true, None).await;

        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("http_proxy");

        assert!(res.is_ok());
        assert_eq!(res.unwrap(), png_bytes);
    }

    #[tokio::test]
    async fn test_download_image_http_hostname_socket_pinning_and_rebinding_protection() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let png_bytes = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x02];
        let png_bytes_clone = png_bytes;

        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf).await;
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\n\r\n",
                    png_bytes_clone.len()
                );
                let _ = stream.write_all(header.as_bytes()).await;
                let _ = stream.write_all(&png_bytes_clone).await;
            }
        });

        // Using an unresolvable mock hostname with port
        let unresolvable_url = format!(
            "http://unresolvable-mock-image-domain.test:{}/pinned.png",
            addr.port()
        );

        // Prove that .resolve() pins the mock hostname to the vetted socket address
        let res =
            commands::download_image_http_test_helper(&unresolvable_url, |_| true, Some(addr))
                .await;

        assert!(res.is_ok());
        assert_eq!(res.unwrap(), png_bytes);
    }

    #[test]
    fn test_parse_meta_token_exchange_response_and_redaction() {
        // Success JSON
        let success_json: serde_json::Value = serde_json::json!({
            "access_token": "EAAB_exchanged_long_lived_token_123",
            "token_type": "bearer"
        });
        let res = commands::parse_meta_token_exchange_response(&success_json);
        assert_eq!(res.unwrap(), "EAAB_exchanged_long_lived_token_123");

        // Error JSON
        let err_json: serde_json::Value = serde_json::json!({
            "error": {
                "message": "Error validating client secret test_secret_val for token EAAB_secret_token",
                "type": "OAuthException",
                "code": 190
            }
        });
        let err_res = commands::parse_meta_token_exchange_response(&err_json);
        assert!(err_res.is_err());
    }

    #[test]
    fn test_map_facebook_legacy_perms_to_scopes() {
        let legacy_admin = serde_json::json!(["ADMINISTER", "CREATE_CONTENT"]);
        let scopes =
            commands::map_facebook_legacy_perms_to_scopes(legacy_admin.as_array().unwrap());
        assert!(scopes.contains(&"pages_show_list".to_string()));
        assert!(scopes.contains(&"pages_messaging".to_string()));
        assert!(scopes.contains(&"pages_manage_posts".to_string()));
        assert!(scopes.contains(&"leads_retrieval".to_string()));
    }

    #[test]
    fn test_redact_exchange_secret() {
        let raw = "Error with clean_token EAAB_token_xyz and app_secret secret_abc123 failed";
        let safe = commands::redact_exchange_secret(raw, "EAAB_token_xyz", "secret_abc123");
        assert_eq!(
            safe,
            "Error with clean_token [REDACTED] and app_secret [REDACTED] failed"
        );
        assert!(!safe.contains("EAAB_token_xyz"));
        assert!(!safe.contains("secret_abc123"));
    }

    #[tokio::test]
    async fn test_concurrent_dotenv_and_fetch_facebook_pages() {
        let mut handles = Vec::new();
        for _ in 0..10 {
            handles.push(tokio::spawn(async {
                crate::load_dotenv_if_present();
                let res = commands::fetch_facebook_pages_from_token_internal("   ", None).await;
                assert!(res.is_err());
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
    }

    #[test]
    fn test_zalo_outreach_skips_leads_without_phone() {
        let mut conn = crate::db::init_db(":memory:").unwrap();

        // 1. Lead with valid mobile phone
        let lead_with_phone = commands::add_lead_internal(
            &mut conn,
            "Lead Valid Phone".into(),
            "Company A".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "lead1@example.com".into(),
            "0901234567".into(),
            None,
            None,
        )
        .unwrap();

        // 2. Lead with empty phone and no zalo_uid
        let lead_no_phone = commands::add_lead_internal(
            &mut conn,
            "Lead No Phone".into(),
            "Company B".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "lead2@example.com".into(),
            "".into(),
            None,
            None,
        )
        .unwrap();

        // 3. Lead with whitespace phone and no zalo_uid
        let lead_ws_phone = commands::add_lead_internal(
            &mut conn,
            "Lead Whitespace Phone".into(),
            "Company C".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "lead3@example.com".into(),
            "   ".into(),
            None,
            None,
        )
        .unwrap();

        // 4. Lead without phone but with cached zalo_uid
        let lead_with_uid = commands::add_lead_internal(
            &mut conn,
            "Lead With UID".into(),
            "Company D".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "lead4@example.com".into(),
            "".into(),
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE leads SET zalo_uid = '1234567890' WHERE id = ?1",
            rusqlite::params![lead_with_uid],
        )
        .unwrap();

        let query = commands::OutreachLeadQuery {
            category_id: None,
            segmentation: "never_sent".to_string(),
            filter_template_id: None,
            channel: "zalo".to_string(),
            search: None,
            filter_verified_zalo: false,
            filter_unreached: false,
            filter_mobile_only: false,
        };

        let page = commands::get_outreach_leads_page_internal(&conn, &query, None, 50).unwrap();
        let returned_ids: Vec<i32> = page.items.into_iter().filter_map(|l| l.id).collect();

        assert!(
            returned_ids.contains(&lead_with_phone),
            "Lead with valid phone should be included"
        );
        assert!(
            returned_ids.contains(&lead_with_uid),
            "Lead with Zalo UID should be included"
        );
        assert!(
            !returned_ids.contains(&lead_no_phone),
            "Lead without phone should be skipped"
        );
        assert!(
            !returned_ids.contains(&lead_ws_phone),
            "Lead with whitespace phone should be skipped"
        );
        assert_eq!(
            page.total, 2,
            "Total count should only include outreachable leads"
        );

        let resolved = commands::resolve_outreach_lead_ids_internal(&conn, &query, &[]).unwrap();
        assert_eq!(resolved.len(), 2);
        assert!(resolved.contains(&lead_with_phone));
        assert!(resolved.contains(&lead_with_uid));

        // Test filter_outreachable_zalo_lead_ids
        let all_ids = vec![lead_with_phone, lead_no_phone, lead_ws_phone, lead_with_uid];
        let (valid_ids, invalid_ids) =
            commands::filter_outreachable_zalo_lead_ids(&conn, &all_ids).unwrap();
        assert_eq!(valid_ids.len(), 2);
        assert_eq!(invalid_ids.len(), 2);
        assert!(valid_ids.contains(&lead_with_phone));
        assert!(valid_ids.contains(&lead_with_uid));
        assert!(invalid_ids.contains(&lead_no_phone));
        assert!(invalid_ids.contains(&lead_ws_phone));

        // Check invalid leads were marked in DB
        let (has_zalo_2, zalo_status_2): (Option<i32>, Option<String>) = conn
            .query_row(
                "SELECT has_zalo, zalo_status FROM leads WHERE id = ?1",
                [lead_no_phone],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(has_zalo_2, Some(0));
        assert_eq!(zalo_status_2.as_deref(), Some("INVALID_PHONE"));
    }

    #[test]
    fn test_zalo_outreach_eligibility_sentinels_and_whitespace() {
        let mut conn = crate::db::init_db(":memory:").unwrap();

        // 1. Whitespace phone only (\t\n) -> EXCLUDED
        let lead_tab_newline = commands::add_lead_internal(
            &mut conn,
            "Tab Newline Phone".into(),
            "Company WS".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "ws@example.com".into(),
            "\t\n".into(),
            None,
            None,
        )
        .unwrap();

        // 2. Sentinel UID "0" -> EXCLUDED
        let lead_uid_zero = commands::add_lead_internal(
            &mut conn,
            "UID Zero".into(),
            "Company Zero".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "zero@example.com".into(),
            "".into(),
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE leads SET zalo_uid = '0' WHERE id = ?1",
            rusqlite::params![lead_uid_zero],
        )
        .unwrap();

        // 3. Sentinel UID "-1" -> EXCLUDED
        let lead_uid_neg = commands::add_lead_internal(
            &mut conn,
            "UID Neg".into(),
            "Company Neg".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "neg@example.com".into(),
            "".into(),
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE leads SET zalo_uid = '-1' WHERE id = ?1",
            rusqlite::params![lead_uid_neg],
        )
        .unwrap();

        // 4. Sentinel UID "null" -> EXCLUDED
        let lead_uid_null = commands::add_lead_internal(
            &mut conn,
            "UID Null".into(),
            "Company Null".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "null@example.com".into(),
            "".into(),
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE leads SET zalo_uid = 'null' WHERE id = ?1",
            rusqlite::params![lead_uid_null],
        )
        .unwrap();

        // 5. Padded sentinel UID " -1 " -> EXCLUDED
        let lead_uid_padded_neg = commands::add_lead_internal(
            &mut conn,
            "Padded Neg".into(),
            "Company Padded Neg".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "pneg@example.com".into(),
            "".into(),
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE leads SET zalo_uid = ' -1 ' WHERE id = ?1",
            rusqlite::params![lead_uid_padded_neg],
        )
        .unwrap();

        // 6. Padded sentinel UID " 0 " -> EXCLUDED
        let lead_uid_padded_zero = commands::add_lead_internal(
            &mut conn,
            "Padded Zero".into(),
            "Company Padded Zero".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "pzero@example.com".into(),
            "".into(),
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE leads SET zalo_uid = ' 0 ' WHERE id = ?1",
            rusqlite::params![lead_uid_padded_zero],
        )
        .unwrap();

        // 7. All-zeros UID "00" -> EXCLUDED
        let lead_uid_double_zero = commands::add_lead_internal(
            &mut conn,
            "Double Zero".into(),
            "Company Double Zero".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "dzero@example.com".into(),
            "".into(),
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE leads SET zalo_uid = '00' WHERE id = ?1",
            rusqlite::params![lead_uid_double_zero],
        )
        .unwrap();

        // 8. Padded sentinel UID " undefined " -> EXCLUDED
        let lead_uid_padded_undefined = commands::add_lead_internal(
            &mut conn,
            "Padded Undefined".into(),
            "Company Undef".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "pundef@example.com".into(),
            "".into(),
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE leads SET zalo_uid = ' undefined ' WHERE id = ?1",
            rusqlite::params![lead_uid_padded_undefined],
        )
        .unwrap();

        // 9. Single digit phone "1" (no UID) -> EXCLUDED
        let lead_phone_one = commands::add_lead_internal(
            &mut conn,
            "Phone One".into(),
            "Company One".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "pone@example.com".into(),
            "1".into(),
            None,
            None,
        )
        .unwrap();

        // 10. Short digit phone "+84-90-123" (no UID) -> EXCLUDED
        let lead_phone_short = commands::add_lead_internal(
            &mut conn,
            "Phone Short".into(),
            "Company Short".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "pshort@example.com".into(),
            "+84-90-123".into(),
            None,
            None,
        )
        .unwrap();

        // 11. Alphabetic phone "abc" (no UID) -> EXCLUDED
        let lead_phone_abc = commands::add_lead_internal(
            &mut conn,
            "Phone Abc".into(),
            "Company Abc".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "pabc@example.com".into(),
            "abc".into(),
            None,
            None,
        )
        .unwrap();

        // 12. Valid mobile phone AND sentinel UID "0" -> INCLUDED (fallback to valid phone)
        let lead_phone_and_zero_uid = commands::add_lead_internal(
            &mut conn,
            "Phone and Zero UID".into(),
            "Company Fallback".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "fallback@example.com".into(),
            "0909999888".into(),
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE leads SET zalo_uid = '0' WHERE id = ?1",
            rusqlite::params![lead_phone_and_zero_uid],
        )
        .unwrap();

        // 13. Numeric UID only -> INCLUDED
        let lead_valid_uid_only = commands::add_lead_internal(
            &mut conn,
            "Valid UID Only".into(),
            "Company UID".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "uid@example.com".into(),
            "".into(),
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE leads SET zalo_uid = '1862258430779885879' WHERE id = ?1",
            rusqlite::params![lead_valid_uid_only],
        )
        .unwrap();

        // 14. String/cached UID only "cached-uid" -> INCLUDED
        let lead_uid_cached = commands::add_lead_internal(
            &mut conn,
            "Cached UID Lead".into(),
            "Company Cached".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "cached@example.com".into(),
            "".into(),
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE leads SET zalo_uid = 'cached-uid' WHERE id = ?1",
            rusqlite::params![lead_uid_cached],
        )
        .unwrap();

        // 15. International formatted mobile "+84 90 123 4567" -> INCLUDED
        let lead_phone_intl_format = commands::add_lead_internal(
            &mut conn,
            "Intl Mobile Lead".into(),
            "Company Intl".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "intl@example.com".into(),
            "+84 90 123 4567".into(),
            None,
            None,
        )
        .unwrap();

        // 16. Invalid phone "1" BUT has valid UID "cached-uid" -> INCLUDED (UID fallback)
        let lead_invalid_phone_with_cached_uid = commands::add_lead_internal(
            &mut conn,
            "Phone One With Cached UID".into(),
            "Company Fallback2".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "fb2@example.com".into(),
            "1".into(),
            None,
            None,
        )
        .unwrap();
        conn.execute(
            "UPDATE leads SET zalo_uid = 'cached-uid' WHERE id = ?1",
            rusqlite::params![lead_invalid_phone_with_cached_uid],
        )
        .unwrap();

        let query = commands::OutreachLeadQuery {
            category_id: None,
            segmentation: "all".to_string(),
            filter_template_id: None,
            channel: "zalo".to_string(),
            search: None,
            filter_verified_zalo: false,
            filter_unreached: false,
            filter_mobile_only: false,
        };

        let page = commands::get_outreach_leads_page_internal(&conn, &query, None, 50).unwrap();
        let ids: Vec<i32> = page.items.into_iter().filter_map(|l| l.id).collect();

        assert_eq!(page.total, 5, "Exactly 5 leads must be eligible in SQL");
        assert!(ids.contains(&lead_phone_and_zero_uid));
        assert!(ids.contains(&lead_valid_uid_only));
        assert!(ids.contains(&lead_uid_cached));
        assert!(ids.contains(&lead_phone_intl_format));
        assert!(ids.contains(&lead_invalid_phone_with_cached_uid));

        assert!(!ids.contains(&lead_tab_newline));
        assert!(!ids.contains(&lead_uid_zero));
        assert!(!ids.contains(&lead_uid_neg));
        assert!(!ids.contains(&lead_uid_null));
        assert!(!ids.contains(&lead_uid_padded_neg));
        assert!(!ids.contains(&lead_uid_padded_zero));
        assert!(!ids.contains(&lead_uid_double_zero));
        assert!(!ids.contains(&lead_uid_padded_undefined));
        assert!(!ids.contains(&lead_phone_one));
        assert!(!ids.contains(&lead_phone_short));
        assert!(!ids.contains(&lead_phone_abc));

        let batch = vec![
            lead_tab_newline,
            lead_uid_zero,
            lead_uid_neg,
            lead_uid_null,
            lead_uid_padded_neg,
            lead_uid_padded_zero,
            lead_uid_double_zero,
            lead_uid_padded_undefined,
            lead_phone_one,
            lead_phone_short,
            lead_phone_abc,
            lead_phone_and_zero_uid,
            lead_valid_uid_only,
            lead_uid_cached,
            lead_phone_intl_format,
            lead_invalid_phone_with_cached_uid,
        ];
        let (valid, invalid) = commands::filter_outreachable_zalo_lead_ids(&conn, &batch).unwrap();
        assert_eq!(valid.len(), 5, "Helper must accept exactly 5 valid leads");
        assert_eq!(
            invalid.len(),
            11,
            "Helper must reject exactly 11 invalid leads"
        );
        assert!(valid.contains(&lead_phone_and_zero_uid));
        assert!(valid.contains(&lead_valid_uid_only));
        assert!(valid.contains(&lead_uid_cached));
        assert!(valid.contains(&lead_phone_intl_format));
        assert!(valid.contains(&lead_invalid_phone_with_cached_uid));

        assert!(invalid.contains(&lead_tab_newline));
        assert!(invalid.contains(&lead_uid_zero));
        assert!(invalid.contains(&lead_uid_neg));
        assert!(invalid.contains(&lead_uid_null));
        assert!(invalid.contains(&lead_uid_padded_neg));
        assert!(invalid.contains(&lead_uid_padded_zero));
        assert!(invalid.contains(&lead_uid_double_zero));
        assert!(invalid.contains(&lead_uid_padded_undefined));
        assert!(invalid.contains(&lead_phone_one));
        assert!(invalid.contains(&lead_phone_short));
        assert!(invalid.contains(&lead_phone_abc));
    }

    #[test]
    fn test_filter_outreachable_zalo_lead_ids_propagates_db_errors() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // Table 'leads' does not exist in this connection
        let res = commands::filter_outreachable_zalo_lead_ids(&conn, &[1, 2]);
        assert!(
            res.is_err(),
            "Missing table must return Err from filter helper"
        );

        // Read-only DB write failure propagation
        let mut rw_conn = crate::db::init_db(":memory:").unwrap();
        let lead_id = commands::add_lead_internal(
            &mut rw_conn,
            "No Phone Lead".into(),
            "Company".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "test@example.com".into(),
            "".into(),
            None,
            None,
        )
        .unwrap();

        // Set DB to query_only (read-only)
        rw_conn.execute_batch("PRAGMA query_only = ON;").unwrap();
        // Helper must attempt to write UPDATE and return Err instead of swallowing it
        let read_only_res = commands::filter_outreachable_zalo_lead_ids(&rw_conn, &[lead_id]);
        assert!(
            read_only_res.is_err(),
            "Read-only DB must return Err when update fails, got: {:?}",
            read_only_res
        );
    }

    #[test]
    fn test_email_sibling_outreach_lead_filter() {
        let mut conn = crate::db::init_db(":memory:").unwrap();

        let lead_valid_email = commands::add_lead_internal(
            &mut conn,
            "Valid Email".into(),
            "Company A".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "valid@example.com".into(),
            "0901234567".into(),
            None,
            None,
        )
        .unwrap();

        let lead_empty_email = commands::add_lead_internal(
            &mut conn,
            "Empty Email".into(),
            "Company B".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "".into(),
            "0901234568".into(),
            None,
            None,
        )
        .unwrap();

        let lead_ws_email = commands::add_lead_internal(
            &mut conn,
            "WS Email".into(),
            "Company C".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "   ".into(),
            "0901234569".into(),
            None,
            None,
        )
        .unwrap();

        let lead_no_at_email = commands::add_lead_internal(
            &mut conn,
            "No At Email".into(),
            "Company D".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "invalid-email".into(),
            "0901234570".into(),
            None,
            None,
        )
        .unwrap();

        let query = commands::OutreachLeadQuery {
            category_id: None,
            segmentation: "all".to_string(),
            filter_template_id: None,
            channel: "email".to_string(),
            search: None,
            filter_verified_zalo: true, // Should be ignored for email
            filter_unreached: false,
            filter_mobile_only: false,
        };

        let page = commands::get_outreach_leads_page_internal(&conn, &query, None, 50).unwrap();
        let ids: Vec<i32> = page.items.into_iter().filter_map(|l| l.id).collect();

        assert_eq!(page.total, 1, "Only valid email must be included");
        assert!(ids.contains(&lead_valid_email));
        assert!(!ids.contains(&lead_empty_email));
        assert!(!ids.contains(&lead_ws_email));
        assert!(!ids.contains(&lead_no_at_email));
    }

    fn outreach_query(channel: &str, segmentation: &str) -> commands::OutreachLeadQuery {
        commands::OutreachLeadQuery {
            category_id: None,
            segmentation: segmentation.to_string(),
            filter_template_id: None,
            channel: channel.to_string(),
            search: None,
            filter_verified_zalo: false,
            filter_unreached: false,
            filter_mobile_only: false,
        }
    }

    fn add_outreach_test_lead(
        conn: &mut rusqlite::Connection,
        name: &str,
        email: &str,
        phone: &str,
    ) -> i32 {
        commands::add_lead_internal(
            conn,
            name.to_string(),
            "Company".to_string(),
            "New".to_string(),
            0.0,
            "Owner".to_string(),
            email.to_string(),
            phone.to_string(),
            None,
            None,
        )
        .unwrap()
    }

    #[test]
    fn test_outreach_channels_keep_their_own_lead_filter() {
        let mut conn = crate::db::init_db(":memory:").unwrap();
        let vn = add_outreach_test_lead(&mut conn, "VN Mobile", "vn@example.com", "0901234567");
        let intl = add_outreach_test_lead(&mut conn, "International", "", "+14155552671");
        let landline =
            add_outreach_test_lead(&mut conn, "Landline", "land@example.com", "02838221234");
        let no_zalo =
            add_outreach_test_lead(&mut conn, "Has Zalo Zero", "nz@example.com", "0901234568");
        let empty = add_outreach_test_lead(&mut conn, "Empty Phone", "empty@example.com", "");
        let spaces = add_outreach_test_lead(&mut conn, "Spaces Phone", "spaces@example.com", "   ");
        let whitespace = add_outreach_test_lead(
            &mut conn,
            "Whitespace Phone",
            "white@example.com",
            "\t\n\r  \t",
        );
        let zalo_only = add_outreach_test_lead(&mut conn, "Zalo Only", "zalo@example.com", "");
        conn.execute("UPDATE leads SET has_zalo = 0 WHERE id = ?1", [no_zalo])
            .unwrap();
        conn.execute("UPDATE leads SET zalo_reached = 1 WHERE id = ?1", [vn])
            .unwrap();
        conn.execute(
            "UPDATE leads SET zalo_uid = '123456789' WHERE id = ?1",
            [zalo_only],
        )
        .unwrap();

        let whatsapp = commands::get_outreach_leads_page_internal(
            &conn,
            &outreach_query("whatsapp", "all"),
            None,
            50,
        )
        .unwrap();
        let whatsapp_ids: Vec<i32> = whatsapp.items.iter().filter_map(|lead| lead.id).collect();
        assert_eq!(whatsapp.total, 4);
        for id in [vn, intl, landline, no_zalo] {
            assert!(whatsapp_ids.contains(&id), "whatsapp must keep lead {id}");
        }
        for id in [empty, spaces, whitespace, zalo_only] {
            assert!(!whatsapp_ids.contains(&id), "whatsapp must drop lead {id}");
        }

        let mut whatsapp_unreached_query = outreach_query("whatsapp", "all");
        whatsapp_unreached_query.filter_unreached = true;
        let whatsapp_unreached =
            commands::get_outreach_leads_page_internal(&conn, &whatsapp_unreached_query, None, 50)
                .unwrap();
        let whatsapp_unreached_ids: Vec<i32> = whatsapp_unreached
            .items
            .iter()
            .filter_map(|lead| lead.id)
            .collect();
        assert_eq!(
            whatsapp_unreached.total, 4,
            "zalo_reached must not filter the WhatsApp channel"
        );
        assert!(whatsapp_unreached_ids.contains(&vn));

        let zalo = commands::get_outreach_leads_page_internal(
            &conn,
            &outreach_query("zalo", "all"),
            None,
            50,
        )
        .unwrap();
        let zalo_ids: Vec<i32> = zalo.items.iter().filter_map(|lead| lead.id).collect();
        assert_eq!(zalo.total, 2, "zalo keeps only VN mobile or a Zalo UID");
        assert!(zalo_ids.contains(&vn));
        assert!(zalo_ids.contains(&zalo_only));
        assert!(!zalo_ids.contains(&intl));
        assert!(!zalo_ids.contains(&landline));
        assert!(!zalo_ids.contains(&no_zalo));

        let email = commands::get_outreach_leads_page_internal(
            &conn,
            &outreach_query("email", "all"),
            None,
            50,
        )
        .unwrap();
        let email_ids: Vec<i32> = email.items.iter().filter_map(|lead| lead.id).collect();
        assert!(email_ids.contains(&vn));
        assert!(!email_ids.contains(&intl), "email keeps its own @ filter");
        assert!(email.items.iter().all(|lead| lead.email.contains('@')));

        let unknown = commands::get_outreach_leads_page_internal(
            &conn,
            &outreach_query("sms", "all"),
            None,
            50,
        );
        assert!(
            unknown.unwrap_err().to_lowercase().contains("invalid"),
            "an unknown channel must map to invalid_request"
        );

        let unsupported_segmentation = commands::get_outreach_leads_page_internal(
            &conn,
            &outreach_query("whatsapp", "never_sent"),
            None,
            50,
        );
        assert!(
            unsupported_segmentation
                .unwrap_err()
                .to_lowercase()
                .contains("invalid"),
            "WhatsApp has no outreach-history segmentation"
        );
    }

    #[test]
    fn test_zca_dob_all_models_both_negative_values() {
        let negative_dobs = [-25200, -72255600];

        for dob_val in negative_dobs {
            // 1. UserBasic
            let json_basic = serde_json::json!({
                "avatar": "https://avatar.url",
                "cover": "https://cover.url",
                "status": "active",
                "gender": 1,
                "dob": dob_val,
                "sdob": "31/12/1969",
                "globalId": "gid_123",
                "bizPkg": { "label": null, "pkgId": 0 },
                "uid": "123456789",
                "zalo_name": "Test User",
                "display_name": "Test User"
            });
            let parsed_basic: Result<zca_rs::models::UserBasic, _> =
                serde_json::from_value(json_basic);
            assert!(
                parsed_basic.is_ok(),
                "UserBasic must deserialize dob={dob_val}, err: {:?}",
                parsed_basic.err()
            );

            // 2. User
            let json_user = serde_json::json!({
                "userId": "123456789",
                "username": "testuser",
                "displayName": "Test User",
                "zaloName": "Test User",
                "avatar": "https://avatar.url",
                "bgavatar": "",
                "cover": "",
                "gender": 1,
                "dob": dob_val,
                "sdob": "18/09/1967",
                "status": "",
                "phoneNumber": "0903733899",
                "isFr": 0,
                "isBlocked": 0,
                "lastActionTime": 0,
                "lastUpdateTime": 0,
                "isActive": 1,
                "key": 0,
                "type": 0,
                "isActivePC": 0,
                "isActiveWeb": 0,
                "isValid": 1,
                "userKey": "",
                "accountStatus": 0,
                "oaInfo": null,
                "user_mode": 0,
                "globalId": "gid_456",
                "bizPkg": { "label": null, "pkgId": 0 },
                "createdTs": 0,
                "oa_status": null
            });
            let parsed_user: Result<zca_rs::models::User, _> = serde_json::from_value(json_user);
            assert!(
                parsed_user.is_ok(),
                "User must deserialize dob={dob_val}, err: {:?}",
                parsed_user.err()
            );

            // 3. FriendRecommendationsDataInfo
            let json_friend_info = serde_json::json!({
                "userId": "123456789",
                "username": "testfriend",
                "displayName": "Test Friend",
                "zaloName": "Test Friend",
                "avatar": "https://avatar.url",
                "phoneNumber": "0866091839",
                "status": "",
                "gender": 1,
                "dob": dob_val,
                "type": 0,
                "recommType": 0,
                "recommSrc": 0,
                "recommTime": 0,
                "recommInfo": { "suggestWay": 0, "source": 0, "message": "" },
                "bizPkg": { "label": null, "pkgId": 0 },
                "isSeenFriendReq": false
            });
            let parsed_friend: Result<zca_rs::apis::friend::FriendRecommendationsDataInfo, _> =
                serde_json::from_value(json_friend_info);
            assert!(
                parsed_friend.is_ok(),
                "FriendRecommendationsDataInfo must deserialize dob={dob_val}, err: {:?}",
                parsed_friend.err()
            );
        }
    }

    #[tokio::test]
    async fn test_zalo_outreach_partitioning_and_runtime_mock_execution() {
        // 1. Test partition_outreach_leads logic (dedup IMEIs and lead IDs, even round-robin)
        let imeis = vec![
            "imei-alpha".to_string(),
            "imei-beta".to_string(),
            "imei-alpha".to_string(),
        ];
        let leads = vec![101, 102, 103, 104, 101, 102];
        let partitions = commands::partition_outreach_leads(&imeis, &leads).unwrap();
        assert_eq!(partitions.len(), 2, "Duplicate IMEIs must be deduplicated");
        assert_eq!(partitions[0].0, "imei-alpha");
        assert_eq!(partitions[0].1, vec![101, 103]);
        assert_eq!(partitions[1].0, "imei-beta");
        assert_eq!(partitions[1].1, vec![102, 104]);

        // 2. Test runtime mock execution with multiple sessions and error handling
        let conn = crate::db::init_db(":memory:").unwrap();
        let conn_mutex = std::sync::Mutex::new(conn);

        // Insert template
        {
            let conn = conn_mutex.lock().unwrap();
            conn.execute(
                "INSERT INTO outreach_templates (id, name, content, created_at) VALUES (1, 'Zalo Promo', 'Hello {{name}}', '2026-09-08T00:00:00Z')",
                [],
            )
            .unwrap();
        }

        // Insert leads: 2 valid, 2 invalid (empty and whitespace)
        let lead1 = {
            let mut conn = conn_mutex.lock().unwrap();
            commands::add_lead_internal(
                &mut conn,
                "Lead One".into(),
                "Company 1".into(),
                "New".into(),
                0.0,
                "Owner".into(),
                "l1@example.com".into(),
                "0901111111".into(),
                None,
                None,
            )
            .unwrap()
        };
        let lead2_invalid_empty = {
            let mut conn = conn_mutex.lock().unwrap();
            commands::add_lead_internal(
                &mut conn,
                "Lead Two".into(),
                "Company 2".into(),
                "New".into(),
                0.0,
                "Owner".into(),
                "l2@example.com".into(),
                "".into(),
                None,
                None,
            )
            .unwrap()
        };
        let lead3_invalid_ws = {
            let mut conn = conn_mutex.lock().unwrap();
            commands::add_lead_internal(
                &mut conn,
                "Lead Three".into(),
                "Company 3".into(),
                "New".into(),
                0.0,
                "Owner".into(),
                "l3@example.com".into(),
                "   \t\n  ".into(),
                None,
                None,
            )
            .unwrap()
        };
        let lead4 = {
            let mut conn = conn_mutex.lock().unwrap();
            commands::add_lead_internal(
                &mut conn,
                "Lead Four".into(),
                "Company 4".into(),
                "New".into(),
                0.0,
                "Owner".into(),
                "l4@example.com".into(),
                "0904444444".into(),
                None,
                None,
            )
            .unwrap()
        };

        // Filter outreachable leads
        let all_input_ids = vec![lead1, lead2_invalid_empty, lead3_invalid_ws, lead4];
        let (valid_ids, skipped_ids) = {
            let conn = conn_mutex.lock().unwrap();
            commands::filter_outreachable_zalo_lead_ids(&conn, &all_input_ids).unwrap()
        };
        assert_eq!(valid_ids, vec![lead1, lead4]);
        assert_eq!(skipped_ids, vec![lead2_invalid_empty, lead3_invalid_ws]);

        // Verify invalid leads marked in DB
        {
            let conn = conn_mutex.lock().unwrap();
            for invalid_id in &skipped_ids {
                let (has_zalo, zalo_status): (Option<i32>, Option<String>) = conn
                    .query_row(
                        "SELECT has_zalo, zalo_status FROM leads WHERE id = ?1",
                        [invalid_id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .unwrap();
                assert_eq!(has_zalo, Some(0));
                assert_eq!(zalo_status.as_deref(), Some("INVALID_PHONE"));
            }
        }

        // Partition the 2 valid leads across 2 mock sessions
        let mock_imeis = vec!["mock-imei-1".to_string(), "mock-imei-2".to_string()];
        let run_partitions = commands::partition_outreach_leads(&mock_imeis, &valid_ids).unwrap();
        assert_eq!(run_partitions.len(), 2);
        assert_eq!(run_partitions[0], ("mock-imei-1".to_string(), vec![lead1]));
        assert_eq!(run_partitions[1], ("mock-imei-2".to_string(), vec![lead4]));

        let options = commands::ZaloOutreachRunOptions {
            enable_burst_limit: false,
            burst_max_messages: 0,
            burst_delay_seconds: 0,
            inter_message_delay_seconds: 0,
        };

        // Execute session 1
        let scope1 = commands::OutreachProgressScope {
            processed_offset: 0,
            sent_offset: 0,
            total_leads: 2,
            emit_complete: false,
        };
        let res1 = commands::send_zalo_outreach_internal(
            NO_APP_HANDLE,
            &conn_mutex,
            "mock-imei-1".to_string(),
            vec![lead1],
            1,
            options,
            scope1,
        )
        .await
        .unwrap();
        assert_eq!(res1.total_sent, 1);

        // Execute session 2
        let scope2 = commands::OutreachProgressScope {
            processed_offset: 1,
            sent_offset: 1,
            total_leads: 2,
            emit_complete: true,
        };
        let res2 = commands::send_zalo_outreach_internal(
            NO_APP_HANDLE,
            &conn_mutex,
            "mock-imei-2".to_string(),
            vec![lead4],
            1,
            options,
            scope2,
        )
        .await
        .unwrap();
        assert_eq!(res2.total_sent, 1);

        // Verify DB updates post-outreach
        {
            let conn = conn_mutex.lock().unwrap();

            // Check leads table updated for both valid leads
            for sent_lead_id in &[lead1, lead4] {
                let (has_zalo, zalo_reached, zalo_status): (i32, i32, Option<String>) = conn
                    .query_row(
                        "SELECT has_zalo, zalo_reached, zalo_status FROM leads WHERE id = ?1",
                        [sent_lead_id],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    )
                    .unwrap();
                assert_eq!(has_zalo, 1);
                assert_eq!(zalo_reached, 1);
                assert_eq!(zalo_status.as_deref(), Some("VERIFIED_EXIST"));
            }

            // Check outreach_history table records both sessions with correct IMEIs
            let history_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM outreach_history WHERE status = 'SUCCESS'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(history_count, 2);

            let session1_record: (i32, String) = conn
                .query_row(
                    "SELECT lead_id, session_imei FROM outreach_history WHERE lead_id = ?1",
                    [lead1],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(session1_record, (lead1, "mock-imei-1".to_string()));

            let session2_record: (i32, String) = conn
                .query_row(
                    "SELECT lead_id, session_imei FROM outreach_history WHERE lead_id = ?1",
                    [lead4],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(session2_record, (lead4, "mock-imei-2".to_string()));

            // Check activities table has 2 entries
            let activity_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM activities WHERE activity_type = 'Outreach'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(activity_count, 2);
        }
    }

    #[tokio::test]
    async fn test_zalo_worker_invalid_phone_write_error_propagation() {
        let mut conn = crate::db::init_db(":memory:").unwrap();
        let template_id =
            crate::db::create_outreach_template(&conn, "Test Template", "Hello {{name}}", None)
                .unwrap();
        let lead_id = commands::add_lead_internal(
            &mut conn,
            "Bad Lead".into(),
            "Company".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "".into(),
            "123".into(), // invalid non-mobile phone, no zalo_uid
            None,
            None,
        )
        .unwrap();

        // Put database into read-only mode so UPDATE will fail
        conn.execute_batch("PRAGMA query_only = ON;").unwrap();
        let db_mutex = std::sync::Mutex::new(conn);

        let res = commands::send_zalo_outreach_internal(
            NO_APP_HANDLE,
            &db_mutex,
            "mock-imei".to_string(),
            vec![lead_id],
            template_id,
            commands::ZaloOutreachRunOptions::default(),
            commands::OutreachProgressScope::default(),
        )
        .await;

        assert!(
            res.is_err(),
            "Worker must propagate DB write error when UPDATE fails"
        );
        let err_msg = res.unwrap_err();
        assert!(
            err_msg.contains("Database write error"),
            "Error must contain 'Database write error', got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_zalo_outreach_events_and_progress_emission() {
        use crate::host::mock::MockApp;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let conn = crate::db::init_db(":memory:").unwrap();
        let mut conn = conn;
        crate::db::seed_data(&mut conn).unwrap();

        let template_id =
            crate::db::create_outreach_template(&conn, "Event Test Tpl", "Hello {{name}}", None)
                .unwrap();

        let lead_id = commands::add_lead_internal(
            &mut conn,
            "Test Lead".into(),
            "Company".into(),
            "New".into(),
            0.0,
            "Owner".into(),
            "".into(),
            "0912345678".into(),
            None,
            None,
        )
        .unwrap();

        let db_state = commands::DbState {
            conn: std::sync::Mutex::new(conn),
        };

        let app = MockApp::new().manage(db_state);

        let log_count = Arc::new(AtomicUsize::new(0));
        let progress_count = Arc::new(AtomicUsize::new(0));
        let complete_count = Arc::new(AtomicUsize::new(0));

        let log_count_clone = log_count.clone();
        app.listen("outreach-log", move |_event| {
            log_count_clone.fetch_add(1, Ordering::SeqCst);
        });

        let progress_count_clone = progress_count.clone();
        app.listen("outreach-progress", move |_event| {
            progress_count_clone.fetch_add(1, Ordering::SeqCst);
        });

        let complete_count_clone = complete_count.clone();
        app.listen("outreach-complete", move |_event| {
            complete_count_clone.fetch_add(1, Ordering::SeqCst);
        });
        let state = app.state::<commands::DbState>();
        let app_handle = app.handle().clone();

        let scope = commands::OutreachProgressScope {
            processed_offset: 0,
            sent_offset: 0,
            total_leads: 1,
            emit_complete: true,
        };

        let res = commands::send_zalo_outreach_internal(
            Some(&app_handle),
            &state.conn,
            "mock-imei-event-test".to_string(),
            vec![lead_id],
            template_id,
            commands::ZaloOutreachRunOptions::default(),
            scope,
        )
        .await;

        assert!(res.is_ok(), "Mock outreach run should succeed: {:?}", res);
        assert_eq!(res.unwrap().total_sent, 1);

        assert!(
            log_count.load(Ordering::SeqCst) >= 1,
            "Must have emitted at least 1 outreach-log event, got {}",
            log_count.load(Ordering::SeqCst)
        );
        assert!(
            progress_count.load(Ordering::SeqCst) >= 1,
            "Must have emitted at least 1 outreach-progress event, got {}",
            progress_count.load(Ordering::SeqCst)
        );
        assert_eq!(
            complete_count.load(Ordering::SeqCst),
            1,
            "Must have emitted exactly 1 outreach-complete event, got {}",
            complete_count.load(Ordering::SeqCst)
        );
    }

    #[test]
    fn language_setting_defaults_to_vi_and_saves_en() {
        let conn = db::init_db(":memory:").unwrap();
        assert_eq!(
            commands::get_language_setting_internal(&conn).unwrap(),
            "vi",
            "a fresh database reads as the product default"
        );

        assert_eq!(
            commands::update_language_setting_internal(&conn, "EN").unwrap(),
            "en"
        );
        assert_eq!(
            commands::get_language_setting_internal(&conn).unwrap(),
            "en"
        );

        assert!(
            commands::update_language_setting_internal(&conn, "fr").is_err(),
            "an unsupported language is refused"
        );
        assert_eq!(
            commands::get_language_setting_internal(&conn).unwrap(),
            "en",
            "a refused save must not change the stored language"
        );

        assert_eq!(
            commands::update_language_setting_internal(&conn, "  ").unwrap(),
            "vi",
            "a blank value falls back to the product default"
        );
    }
}
