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
        assert_eq!(leads.len(), 2);
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
        ).unwrap();
        assert!(new_lead_id > 0);

        let leads_updated = commands::get_leads_internal(&conn).unwrap();
        assert_eq!(leads_updated.len(), 3);

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
        commands::add_activity_internal(&mut conn, 1, "Call".to_string(), "Intro call".to_string(), "Alex Rivera".to_string()).unwrap();
        let activities_updated = commands::get_activities_internal(&conn, 1).unwrap();
        assert_eq!(activities_updated.len(), 2);
        assert_eq!(activities_updated[1].activity_type, "Call");
    }

    #[tokio::test]
    async fn test_google_maps_scrape_command() {
        use crate::scrapers::browser::BrowserManager;
        use crate::scrapers::google_maps::GoogleMapsScraper;
        use std::sync::Mutex;
        use tokio::net::TcpListener;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
                              <button data-item-id="phone:tel:+84111222333">Phone</button>
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
        ).await;

        assert!(res.is_ok(), "Scrape task failed: {:?}", res.err());

        // Check if database lead has been inserted
        let conn_locked = db_mutex.lock().unwrap();
        let leads = commands::get_leads_internal(&conn_locked).unwrap();
        assert_eq!(leads.len(), 3);

        let scraped_lead = leads.iter().find(|l| l.name == "Scraped Cafe");
        assert!(scraped_lead.is_some(), "Scraped Cafe not found in database");
        let lead = scraped_lead.unwrap();
        assert_eq!(lead.phone, "+84111222333");
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
        let dummy_cookies = vec![
            zca_rs::context::ZaloCookie {
                name: "zpw_sek".to_string(),
                value: "sek-123456".to_string(),
                domain: "zalo.me".to_string(),
                path: "/".to_string(),
            }
        ];
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
        
        let cookies: Vec<zca_rs::context::ZaloCookie> = serde_json::from_str(&saved_sessions[0].cookies_json).unwrap();
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0].name, "zpw_sek");
        assert_eq!(cookies[0].value, "sek-123456");

        // 7. Test delete session
        commands::delete_zalo_session_internal(&conn, "imei-999").unwrap();
        let sessions_after_delete = commands::get_zalo_sessions_internal(&conn).unwrap();
        assert!(sessions_after_delete.is_empty());
    }
}

