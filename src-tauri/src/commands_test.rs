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

    #[tokio::test]
    async fn test_google_maps_scrape_command() {
        use crate::scrapers::browser::BrowserManager;
        use crate::scrapers::google_maps::GoogleMapsScraper;
        use std::sync::Mutex;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        // Initialize a memory database
        let mut conn = db::init_db(":memory:").unwrap();
        db::seed_data(&mut conn).unwrap();
        let db_mutex = Mutex::new(conn);

        // Spawn mock server
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0; 4096];
                    if let Ok(n) = socket.read(&mut buf).await {
                        let request = String::from_utf8_lossy(&buf[..n]);
                        let response = if request.contains("/search") {
                            let body = format!(
                                r#"<html><body><div role="feed">
                                  <a href="http://127.0.0.1:{}/maps/place/Scraped+Cafe">Scraped Cafe</a>
                                </div></body></html>"#,
                                port
                            );
                            format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
                        } else if request.contains("/maps/place/Scraped+Cafe") {
                            let body = r#"<html><body>
                              <h1>Scraped Cafe</h1>
                              <button data-item-id="phone:tel:+84911222333">Phone</button>
                              <button data-item-id="address">789 Dong Khoi St, HCMC</button>
                              <a data-item-id="authority" href="https://scrapedcafe.com">Website</a>
                              <a href="https://facebook.com/scrapedcafe">Facebook</a>
                              <a href="https://zalo.me/scrapedcafe">Zalo</a>
                            </body></html>"#;
                            format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
                        } else {
                            format!("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")
                        };
                        let _ = socket.write_all(response.as_bytes()).await;
                    }
                });
            }
        });

        let mock_base = format!("http://127.0.0.1:{}", port);

        // Initialize BrowserManager
        let browser_port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let manager = std::sync::Arc::new(BrowserManager::new(1).with_port(browser_port));
        manager.start(true).await.expect("Failed to start browser");

        let session = manager.acquire().await.expect("Failed to acquire session");
        let scraper = GoogleMapsScraper::with_base_url(session, mock_base);

        let res = commands::run_google_maps_scrape_task(
            None,
            &db_mutex,
            manager.clone(),
            scraper,
            "cafe".to_string(),
            "text".to_string(),
            Some("District 1".to_string()),
            None,
            None,
            None,
        )
        .await;

        assert!(res.is_ok(), "Scrape task failed: {:?}", res.err());

        // Check if database lead has been inserted
        let conn_locked = db_mutex.lock().unwrap();
        let leads = commands::get_leads_internal(&conn_locked).unwrap();
        assert_eq!(leads.len(), 4);

        let scraped_lead = leads.iter().find(|l| l.name == "Scraped Cafe");
        assert!(scraped_lead.is_some(), "Scraped Cafe not found in database");
        let lead = scraped_lead.unwrap();
        assert_eq!(lead.phone, "+84911222333");
        assert_eq!(lead.company, "Scraped Cafe");
        assert_eq!(lead.status, "New");

        let about = lead.about.as_ref().unwrap();
        assert!(about.contains("Address: 789 Dong Khoi St, HCMC"));
        assert!(about.contains("Website: https://scrapedcafe.com"));
        assert!(about.contains("Facebook: https://facebook.com/scrapedcafe"));
        assert!(about.contains("Zalo: https://zalo.me/scrapedcafe"));

        let _ = manager.close().await;
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
        use crate::models::ZaloSessionStatus;
        use crate::models::ZaloSession;

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
        };

        let verify_res = commands::verify_zalo_session_internal(invalid_session).await.unwrap();
        assert_eq!(verify_res.imei, "imei-bad-cookies");
        assert!(!verify_res.is_valid);
        assert!(verify_res.error_message.unwrap().contains("Invalid cookies format"));
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
        let all_statuses = commands::verify_all_zalo_sessions(state.clone()).await.unwrap();
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
            db::create_outreach_template(&conn, "Template Welcome", "Hello [name]!").unwrap();

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
        )
        .await;

        assert!(res.is_ok());

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
        assert_eq!(lead_4.zalo_reached, 0);
    }

    #[tokio::test]
    async fn test_google_maps_scrape_deep_scan() {
        use crate::scrapers::browser::BrowserManager;
        use crate::scrapers::google_maps::GoogleMapsScraper;
        use std::sync::Mutex;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        // Initialize a memory database
        let mut conn = db::init_db(":memory:").unwrap();
        db::seed_data(&mut conn).unwrap();
        let db_mutex = Mutex::new(conn);

        // Spawn mock server
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0; 4096];
                    if let Ok(n) = socket.read(&mut buf).await {
                        let request = String::from_utf8_lossy(&buf[..n]);
                        let response = if request.contains("/search") {
                            let body = format!(
                                r#"<html><body><div role="feed">
                                  <a href="http://127.0.0.1:{}/maps/place/Scraped+Cafe">Scraped Cafe</a>
                                </div></body></html>"#,
                                port
                            );
                            format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
                        } else if request.contains("/maps/place/Scraped+Cafe") {
                            let body = r#"<html><body>
                              <h1>Scraped Cafe</h1>
                              <button data-item-id="phone:tel:+84911222333">Phone</button>
                              <button data-item-id="address">789 Dong Khoi St, HCMC</button>
                              <a data-item-id="authority" href="https://scrapedcafe.com">Website</a>
                              <a href="https://facebook.com/scrapedcafe">Facebook</a>
                              <a href="https://zalo.me/scrapedcafe">Zalo</a>
                            </body></html>"#;
                            format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
                        } else {
                            format!("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")
                        };
                        let _ = socket.write_all(response.as_bytes()).await;
                    }
                });
            }
        });

        let mock_base = format!("http://127.0.0.1:{}", port);

        // Initialize BrowserManager
        let browser_port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let manager = std::sync::Arc::new(BrowserManager::new(1).with_port(browser_port));
        manager.start(true).await.expect("Failed to start browser");

        let session = manager.acquire().await.expect("Failed to acquire session");
        let scraper = GoogleMapsScraper::with_base_url(session, mock_base);

        let res = commands::run_google_maps_scrape_task(
            None,
            &db_mutex,
            manager.clone(),
            scraper,
            "cafe".to_string(),
            "text".to_string(),
            Some("Hồ Chí Minh".to_string()),
            None,
            None,
            Some(vec!["Quận 1".to_string(), "Quận 3".to_string()]),
        )
        .await;

        assert!(res.is_ok(), "Scrape task failed: {:?}", res.err());

        let conn_locked = db_mutex.lock().unwrap();
        let leads = commands::get_leads_internal(&conn_locked).unwrap();
        assert!(leads.len() > 0);

        let scraped_lead = leads.iter().find(|l| l.name == "Scraped Cafe");
        assert!(scraped_lead.is_some(), "Scraped Cafe not found in database");
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
        assert_eq!(res_whitespace.err().unwrap(), "Category name cannot be empty");

        // 3. Create a valid category
        let category_id = commands::create_category(state.clone(), "Partners".to_string()).unwrap();
        assert!(category_id > 0);

        // 4. Verify get_categories now returns the category
        let categories2 = commands::get_categories(state.clone()).unwrap();
        assert_eq!(categories2.len(), 1);
        assert_eq!(categories2[0].name, "Partners");
        assert_eq!(categories2[0].id, Some(category_id));

        // 5. Failure path: adding a non-existent lead ID (e.g. 99999) should return an error
        let res_nonexistent = commands::add_leads_to_category(state.clone(), vec![99999], category_id);
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
            db::create_outreach_template(&conn, "Campaign Template", "Hello {name}!").unwrap();

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
        ).unwrap();

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
        ).unwrap();

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
        ).unwrap();
        
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
        ).unwrap();
        
        // Add all to category
        db::add_lead_to_category(&conn, lead_1, category_id).unwrap();
        db::add_lead_to_category(&conn, lead_2, category_id).unwrap();
        db::add_lead_to_category(&conn, lead_3, category_id).unwrap();
        db::add_lead_to_category(&conn, lead_4, category_id).unwrap();
        
        // Manually set cached uid for lead 4
        conn.execute("UPDATE leads SET zalo_uid = 'cached-123' WHERE id = ?1", [lead_4]).unwrap();

        let db_mutex = Mutex::new(conn);

        let session_imeis = vec!["mock-fail-imei".to_string(), "mock-success-imei".to_string()];

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
        let h1: i64 = conn_locked.query_row("SELECT COUNT(*) FROM outreach_history WHERE lead_id = ?1 AND status = 'FAILED'", [lead_1], |row| row.get(0)).unwrap();
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
        let h4: i64 = conn_locked.query_row("SELECT COUNT(*) FROM outreach_history WHERE lead_id = ?1 AND status = 'SUCCESS'", [lead_4], |row| row.get(0)).unwrap();
        assert_eq!(h4, 1);
    }

    #[tokio::test]
    async fn test_send_zalo_campaign_continues_after_lead_send_failure() {
        use std::sync::Mutex;

        let mut conn = db::init_db(":memory:").unwrap();
        db::seed_data(&mut conn).unwrap();
        let template_id =
            db::create_outreach_template(&conn, "Campaign Template", "Hello [name]!").unwrap();
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
            db::create_outreach_template(&conn, "Campaign Template", "Hello {name}!").unwrap();
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
        ).unwrap();

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
        ).unwrap();

        db::add_lead_to_category(&conn, lead_1, category_id).unwrap();
        db::add_lead_to_category(&conn, lead_2, category_id).unwrap();

        // Lead 2 has cached uid to bypass lookup
        conn.execute("UPDATE leads SET zalo_uid = 'cached-123' WHERE id = ?1", [lead_2]).unwrap();

        let db_mutex = Mutex::new(conn);

        // mock-fail-imei is index 0, mock-success-imei is index 1
        let session_imeis = vec!["mock-fail-imei".to_string(), "mock-success-imei".to_string()];

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
            db::create_outreach_template(&conn, "Campaign Template", "Hello {name}!").unwrap();
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
        ).unwrap();

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
        ).unwrap();

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

        let h2_count: i64 = conn_locked.query_row(
            "SELECT COUNT(*) FROM outreach_history WHERE lead_id = ?1",
            [lead_2],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(h2_count, 0);
    }
}
