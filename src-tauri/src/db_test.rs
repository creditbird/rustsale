#[cfg(test)]
mod tests {
    use super::super::db;

    #[test]
    fn test_db_init_and_tables() {
        let conn = db::init_db(":memory:").expect("Failed to init db");

        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap();
        let tables: Vec<String> = stmt
            .query_map([], |row: &rusqlite::Row| row.get::<_, String>(0))
            .unwrap()
            .map(|r: Result<String, rusqlite::Error>| r.unwrap())
            .collect();

        assert!(tables.contains(&"leads".to_string()));
        assert!(tables.contains(&"activities".to_string()));
        assert!(tables.contains(&"deals".to_string()));
        assert!(tables.contains(&"zalo_sessions".to_string()));
        assert!(tables.contains(&"outreach_templates".to_string()));
        assert!(tables.contains(&"categories".to_string()));
        assert!(tables.contains(&"lead_categories".to_string()));
        assert!(tables.contains(&"outreach_history".to_string()));
        assert!(tables.contains(&"campaigns".to_string()));
        assert!(tables.contains(&"campaign_steps".to_string()));
        assert!(tables.contains(&"campaign_executions".to_string()));
        assert!(tables.contains(&"lead_journey_states".to_string()));
        assert!(tables.contains(&"ai_evidence_logs".to_string()));
        assert!(tables.contains(&"ai_action_proposals".to_string()));

        let zalo_columns: Vec<String> = conn
            .prepare("PRAGMA table_info(zalo_sessions)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert!(zalo_columns.contains(&"auth_status".to_string()));
        assert!(zalo_columns.contains(&"auth_checked_at".to_string()));
        assert!(zalo_columns.contains(&"auth_error".to_string()));
    }

    #[test]
    fn legacy_zalo_session_schema_upgrades_before_duplicate_reconciliation() {
        let path = std::env::temp_dir().join(format!(
            "rustsale-legacy-zalo-upgrade-{}.db",
            rand::random::<u64>()
        ));
        let path_str = path.to_string_lossy().to_string();

        let conn = db::init_db(&path_str).expect("initial database setup succeeds");
        conn.execute(
            "DROP INDEX IF EXISTS idx_channel_accounts_zalo_self_ext_id",
            [],
        )
        .unwrap();
        conn.execute("DROP TABLE zalo_sessions", []).unwrap();
        conn.execute_batch(
            "CREATE TABLE zalo_sessions (
                imei TEXT PRIMARY KEY,
                user_agent TEXT NOT NULL,
                cookies_json TEXT NOT NULL,
                language TEXT NOT NULL,
                display_name TEXT,
                avatar TEXT,
                auth_status TEXT NOT NULL DEFAULT 'unknown',
                auth_checked_at TEXT,
                auth_error TEXT
             );
             INSERT INTO zalo_sessions
                (imei, user_agent, cookies_json, language, display_name)
             VALUES ('imei-new', 'UA/1.0', '[]', 'vi', 'Zalo Account');
             INSERT INTO channel_accounts
                (id, channel, provider_account_key, self_external_id, display_name, connection_status, capabilities_json, created_at, updated_at)
             VALUES
                (100, 'zalo', 'uid-legacy', 'uid-legacy', 'Canonical', 'disconnected', '{}', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z'),
                (200, 'zalo', 'imei-new', 'uid-legacy', 'Relogin', 'connected', '{}', '2026-08-02T00:00:00Z', '2026-08-02T00:00:00Z');
             INSERT INTO channel_conversations
                (id, channel_account_id, external_thread_id, kind, status, created_at, updated_at)
             VALUES
                (1000, 100, 'peer-existing', 'direct', 'active', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z');",
        )
        .unwrap();
        drop(conn);

        let upgraded = db::init_db(&path_str).expect("legacy Zalo schema must upgrade safely");
        let columns: Vec<String> = upgraded
            .prepare("PRAGMA table_info(zalo_sessions)")
            .unwrap()
            .query_map([], |row| row.get(1))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert!(columns.contains(&"zalo_uid".to_string()));
        assert!(columns.contains(&"channel_account_id".to_string()));

        let mapping: (Option<String>, Option<i64>) = upgraded
            .query_row(
                "SELECT zalo_uid, channel_account_id FROM zalo_sessions WHERE imei = 'imei-new'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(mapping, (Some("uid-legacy".to_string()), Some(100)));
        assert_eq!(
            upgraded
                .query_row(
                    "SELECT COUNT(*) FROM channel_accounts WHERE channel = 'zalo' AND self_external_id = 'uid-legacy'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );

        drop(upgraded);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }

    #[test]
    fn smtp_update_with_empty_password_preserves_secret() {
        let conn = db::init_db(":memory:").expect("Failed to init db");
        let server = crate::models::SmtpServer {
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
        };

        let id = db::save_smtp_server(&conn, &server).unwrap();
        let mut update = server;
        update.id = Some(id);
        update.name = "Primary SMTP Updated".to_string();
        update.password.clear();
        db::save_smtp_server(&conn, &update).unwrap();

        let saved = db::get_smtp_server_by_id(&conn, id).unwrap().unwrap();
        assert_eq!(saved.name, "Primary SMTP Updated");
        assert_eq!(saved.password, "secret-password");
    }

    #[test]
    fn test_db_seeding() {
        let mut conn = db::init_db(":memory:").expect("Failed to init db");
        db::seed_data(&mut conn).expect("Failed to seed db");

        // Verify leads
        let lead_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM leads", [], |row| row.get(0))
            .unwrap();
        assert_eq!(lead_count, 3);

        let lead_name: String = conn
            .query_row("SELECT name FROM leads WHERE id = 1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(lead_name, "Sarah Harrington");

        // Verify new fields on Sarah Harrington (id = 1)
        let address1: Option<String> = conn
            .query_row("SELECT address FROM leads WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(address1, Some("123 Main St, New York".to_string()));

        let has_zalo1: Option<i32> = conn
            .query_row("SELECT has_zalo FROM leads WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(has_zalo1, Some(1));

        let zalo_reached1: i32 = conn
            .query_row("SELECT zalo_reached FROM leads WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(zalo_reached1, 0);

        // Verify new fields on Jordan Smith (id = 2)
        let address2: Option<String> = conn
            .query_row("SELECT address FROM leads WHERE id = 2", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(address2, Some("456 Oak Ave, Boston".to_string()));

        let has_zalo2: Option<i32> = conn
            .query_row("SELECT has_zalo FROM leads WHERE id = 2", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(has_zalo2, None); // NULL in database

        let zalo_reached2: i32 = conn
            .query_row("SELECT zalo_reached FROM leads WHERE id = 2", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(zalo_reached2, 0);

        // Verify lele (id = 3)
        let lead3_name: String = conn
            .query_row("SELECT name FROM leads WHERE id = 3", [], |row| row.get(0))
            .unwrap();
        assert_eq!(lead3_name, "lele");

        let address3: Option<String> = conn
            .query_row("SELECT address FROM leads WHERE id = 3", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(address3, Some("789 Pine Rd, Seattle".to_string()));

        let has_zalo3: Option<i32> = conn
            .query_row("SELECT has_zalo FROM leads WHERE id = 3", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(has_zalo3, Some(1));

        let zalo_reached3: i32 = conn
            .query_row("SELECT zalo_reached FROM leads WHERE id = 3", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(zalo_reached3, 1);

        // Verify deals
        let deal_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM deals", [], |row| row.get(0))
            .unwrap();
        assert_eq!(deal_count, 2);

        // Verify activities
        let activity_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM activities", [], |row| row.get(0))
            .unwrap();
        assert_eq!(activity_count, 1);
    }

    #[test]
    fn test_zalo_sessions_and_templates_operations() {
        let conn = db::init_db(":memory:").expect("Failed to init db");

        // Test inserting a Zalo session
        conn.execute(
            "INSERT INTO zalo_sessions (imei, user_agent, cookies_json, language) VALUES (?, ?, ?, ?)",
            [
                "123456789012345",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
                "[]",
                "vi",
            ],
        ).unwrap();

        // Verify session retrieval
        let imei: String = conn
            .query_row(
                "SELECT imei FROM zalo_sessions WHERE language = 'vi'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(imei, "123456789012345");

        // Test inserting an outreach template
        conn.execute(
            "INSERT INTO outreach_templates (name, content, created_at) VALUES (?, ?, ?)",
            [
                "Welcome Template",
                "Hello, {{name}}!",
                "2026-07-17T20:00:00Z",
            ],
        )
        .unwrap();

        // Verify template retrieval
        let content: String = conn
            .query_row(
                "SELECT content FROM outreach_templates WHERE name = 'Welcome Template'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(content, "Hello, {{name}}!");
    }

    #[test]
    fn saving_refreshed_zalo_credentials_preserves_profile_and_account_mapping() {
        let conn = db::init_db(":memory:").expect("Failed to init db");
        conn.execute(
            "INSERT INTO channel_accounts
             (id, channel, provider_account_key, self_external_id, display_name, connection_status, capabilities_json, created_at, updated_at)
             VALUES (77, 'zalo', 'uid-77', 'uid-77', 'Mapped Account', 'connected', '{}', '2026-08-09T00:00:00Z', '2026-08-09T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO zalo_sessions
             (imei, user_agent, cookies_json, language, display_name, avatar, auth_status, zalo_uid, channel_account_id)
             VALUES ('imei-77', 'old-agent', '[{\"old\":true}]', 'vi', 'Mapped Account', 'avatar.png', 'valid', 'uid-77', 77)",
            [],
        )
        .unwrap();

        db::save_zalo_session(&conn, "imei-77", "new-agent", "[{\"new\":true}]", "vi").unwrap();

        let saved = conn
            .query_row(
                "SELECT user_agent, cookies_json, display_name, avatar, auth_status, zalo_uid, channel_account_id
                 FROM zalo_sessions WHERE imei = 'imei-77'",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            saved,
            (
                "new-agent".to_string(),
                "[{\"new\":true}]".to_string(),
                Some("Mapped Account".to_string()),
                Some("avatar.png".to_string()),
                "valid".to_string(),
                Some("uid-77".to_string()),
                Some(77),
            )
        );
    }

    #[test]
    fn test_outreach_template_crud() {
        let conn = db::init_db(":memory:").expect("Failed to init db");

        // 1. Initially empty
        let templates = db::get_outreach_templates(&conn).unwrap();
        assert!(templates.is_empty());

        // 2. Create template
        let id1 =
            db::create_outreach_template(&conn, "Template One", "Hello {name}", None).unwrap();
        assert!(id1 > 0);

        let id2 = db::create_outreach_template(&conn, "Template Two", "Hi {name}", None).unwrap();
        assert!(id2 > 0);

        // 3. Get all templates
        let templates = db::get_outreach_templates(&conn).unwrap();
        assert_eq!(templates.len(), 2);
        assert_eq!(templates[0].name, "Template One");
        assert_eq!(templates[0].content, "Hello {name}");

        // 4. Update template
        db::update_outreach_template(&conn, id1, "Template One Updated", "Hello {name}!", None)
            .unwrap();
        let templates = db::get_outreach_templates(&conn).unwrap();
        let t1 = templates.iter().find(|t| t.id == Some(id1)).unwrap();
        assert_eq!(t1.name, "Template One Updated");
        assert_eq!(t1.content, "Hello {name}!");

        // 5. Delete template
        db::delete_outreach_template(&conn, id2).unwrap();
        let templates = db::get_outreach_templates(&conn).unwrap();
        assert_eq!(templates.len(), 1);
        assert!(templates.iter().all(|t| t.id != Some(id2)));
    }

    #[test]
    fn test_zalo_session_helpers() {
        let conn = db::init_db(":memory:").expect("Failed to init db");

        // Initially no sessions
        let sessions = db::get_all_zalo_sessions(&conn).unwrap();
        assert!(sessions.is_empty());

        // Test save session
        db::save_zalo_session(&conn, "imei-123", "UA-test", "[]", "en").unwrap();

        // Get by IMEI
        let s1 = db::get_zalo_session(&conn, "imei-123").unwrap().unwrap();
        assert_eq!(s1.imei, "imei-123");
        assert_eq!(s1.user_agent, "UA-test");
        assert_eq!(s1.cookies_json, "[]");
        assert_eq!(s1.language, "en");

        // Save again (should update/replace)
        db::save_zalo_session(&conn, "imei-123", "UA-new", "[{}]", "vi").unwrap();
        let s2 = db::get_zalo_session(&conn, "imei-123").unwrap().unwrap();
        assert_eq!(s2.user_agent, "UA-new");
        assert_eq!(s2.cookies_json, "[{}]");
        assert_eq!(s2.language, "vi");

        // Save another session
        db::save_zalo_session(&conn, "imei-456", "UA-other", "[]", "en").unwrap();

        // List all
        let all = db::get_all_zalo_sessions(&conn).unwrap();
        assert_eq!(all.len(), 2);

        // Delete one
        db::delete_zalo_session(&conn, "imei-123").unwrap();
        assert!(db::get_zalo_session(&conn, "imei-123").unwrap().is_none());

        // List all again
        let all_after = db::get_all_zalo_sessions(&conn).unwrap();
        assert_eq!(all_after.len(), 1);
        assert_eq!(all_after[0].imei, "imei-456");

        // Test lookup, update, and delete by zalo_uid and channel_account provider_account_key
        conn.execute(
            "INSERT INTO zalo_sessions (imei, user_agent, cookies_json, language, zalo_uid) VALUES ('imei-uid-test', 'UA', '[]', 'vi', 'uid-999')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO channel_accounts (channel, provider_account_key, created_at, updated_at) VALUES ('zalo', 'uid-999', '2026-01-01', '2026-01-01')",
            [],
        ).unwrap();

        // Get by zalo_uid / provider_account_key
        let s_by_uid = db::get_zalo_session(&conn, "uid-999").unwrap().unwrap();
        assert_eq!(s_by_uid.imei, "imei-uid-test");
        assert_eq!(s_by_uid.zalo_uid.as_deref(), Some("uid-999"));

        // Update auth status by provider_account_key
        db::update_zalo_session_auth_status(&conn, "uid-999", "valid", None).unwrap();
        let s_updated = db::get_zalo_session(&conn, "uid-999").unwrap().unwrap();
        assert_eq!(s_updated.imei, "imei-uid-test");

        // Delete by provider_account_key / zalo_uid
        db::delete_zalo_session(&conn, "uid-999").unwrap();
        assert!(db::get_zalo_session(&conn, "uid-999").unwrap().is_none());
        assert!(db::get_zalo_session(&conn, "imei-uid-test")
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_zalo_sessions_deduplication_returns_latest_session() {
        let conn = db::init_db(":memory:").expect("Failed to init db");

        // Insert multiple sessions with different IMEIs for the same zalo_uid (simulating multiple QR scans)
        conn.execute(
            "INSERT INTO zalo_sessions (imei, user_agent, cookies_json, language, zalo_uid, display_name)
             VALUES ('imei-old-1', 'UA', '[]', 'vi', 'uid-123', 'Old Account 1')",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO zalo_sessions (imei, user_agent, cookies_json, language, zalo_uid, display_name)
             VALUES ('imei-old-2', 'UA', '[]', 'vi', 'uid-123', 'Old Account 2')",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO zalo_sessions (imei, user_agent, cookies_json, language, zalo_uid, display_name)
             VALUES ('imei-latest', 'UA', '[]', 'vi', 'uid-123', 'Latest Account')",
            [],
        ).unwrap();

        // Also insert an independent account with different uid
        conn.execute(
            "INSERT INTO zalo_sessions (imei, user_agent, cookies_json, language, zalo_uid, display_name)
             VALUES ('imei-other', 'UA', '[]', 'vi', 'uid-456', 'Other Account')",
            [],
        ).unwrap();

        // Insert two distinct sessions with empty-string zalo_uid (e.g. pending QR profile sync)
        conn.execute(
            "INSERT INTO zalo_sessions (imei, user_agent, cookies_json, language, zalo_uid, display_name)
             VALUES ('imei-empty-1', 'UA', '[]', 'vi', '', 'Pending Account 1')",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO zalo_sessions (imei, user_agent, cookies_json, language, zalo_uid, display_name)
             VALUES ('imei-empty-2', 'UA', '[]', 'vi', '  ', 'Pending Account 2')",
            [],
        ).unwrap();

        // Insert two distinct sessions with NULL zalo_uid
        conn.execute(
            "INSERT INTO zalo_sessions (imei, user_agent, cookies_json, language, display_name)
             VALUES ('imei-null-1', 'UA', '[]', 'vi', 'Null Account 1')",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO zalo_sessions (imei, user_agent, cookies_json, language, display_name)
             VALUES ('imei-null-2', 'UA', '[]', 'vi', 'Null Account 2')",
            [],
        )
        .unwrap();

        let all = db::get_all_zalo_sessions(&conn).unwrap();
        // 1 deduplicated (uid-123) + 1 (uid-456) + 2 (empty uid) + 2 (null uid) = 6 accounts
        assert_eq!(
            all.len(),
            6,
            "Must deduplicate by valid account uid without collapsing empty/null UIDs"
        );

        let s123 = all
            .iter()
            .find(|s| s.zalo_uid.as_deref() == Some("uid-123"))
            .unwrap();
        assert_eq!(
            s123.imei, "imei-latest",
            "Must return the latest inserted session for uid-123"
        );
        assert_eq!(s123.display_name.as_deref(), Some("Latest Account"));

        let s456 = all
            .iter()
            .find(|s| s.zalo_uid.as_deref() == Some("uid-456"))
            .unwrap();
        assert_eq!(s456.imei, "imei-other");

        assert!(all.iter().any(|s| s.imei == "imei-empty-1"));
        assert!(all.iter().any(|s| s.imei == "imei-empty-2"));
        assert!(all.iter().any(|s| s.imei == "imei-null-1"));
        assert!(all.iter().any(|s| s.imei == "imei-null-2"));
    }

    #[test]
    fn test_wal_mode_and_synchronous() {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join("test_wal.db");
        let db_path_str = db_path.to_str().unwrap();
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(temp_dir.join("test_wal.db-wal"));
        let _ = std::fs::remove_file(temp_dir.join("test_wal.db-shm"));

        let conn = db::init_db(db_path_str).expect("Failed to init db");
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode;", [], |row| row.get(0))
            .unwrap();
        assert_eq!(journal_mode, "wal");

        let synchronous: i32 = conn
            .query_row("PRAGMA synchronous;", [], |row| row.get(0))
            .unwrap();
        assert_eq!(synchronous, 1); // 1 = NORMAL in SQLite

        drop(conn);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(temp_dir.join("test_wal.db-wal"));
        let _ = std::fs::remove_file(temp_dir.join("test_wal.db-shm"));
    }

    #[test]
    fn test_categories_crud() {
        let conn = db::init_db(":memory:").expect("Failed to init db");

        let cat_id = db::create_category(&conn, "VIP Clients").unwrap();
        assert!(cat_id > 0);

        assert!(db::create_category(&conn, "VIP Clients").is_err());

        let cats = db::get_categories(&conn).unwrap();
        assert_eq!(cats.len(), 1);
        assert_eq!(cats[0].name, "VIP Clients");
        assert_eq!(cats[0].id, Some(cat_id));

        db::delete_category(&conn, cat_id).unwrap();
        let cats_after = db::get_categories(&conn).unwrap();
        assert!(cats_after.is_empty());
    }

    #[test]
    fn test_lead_categories_association() {
        let conn = db::init_db(":memory:").expect("Failed to init db");

        conn.execute(
            "INSERT INTO leads (name, company, status, last_activity, owner, email, phone) VALUES (?, ?, ?, ?, ?, ?, ?)",
            [
                "John Doe",
                "Company A",
                "New",
                "2026-07-17T20:00:00Z",
                "Alex",
                "john@example.com",
                "123456",
            ],
        ).unwrap();
        let lead_id = conn.last_insert_rowid() as i32;

        let cat_id = db::create_category(&conn, "Hot Leads").unwrap();

        // 1. Success case
        db::add_lead_to_category(&conn, lead_id, cat_id).unwrap();

        // 2. Duplicate case: ON CONFLICT DO NOTHING should succeed and return Ok(())
        db::add_lead_to_category(&conn, lead_id, cat_id)
            .expect("Duplicate insert failed but should be ignored");

        // 3. FK violation: non-existent lead_id should throw error
        assert!(db::add_lead_to_category(&conn, 99999, cat_id).is_err());

        // 4. FK violation: non-existent category_id should throw error
        assert!(db::add_lead_to_category(&conn, lead_id, 99999).is_err());

        let leads = db::get_leads_by_category(&conn, cat_id).unwrap();
        assert_eq!(leads.len(), 1);
        assert_eq!(leads[0].name, "John Doe");
        assert_eq!(leads[0].zalo_status.as_deref(), Some("UNTESTED"));

        let lead_cats = db::get_categories_for_lead(&conn, lead_id).unwrap();
        assert_eq!(lead_cats.len(), 1);
        assert_eq!(lead_cats[0].name, "Hot Leads");
        assert_eq!(lead_cats[0].id, Some(cat_id));

        db::remove_lead_from_category(&conn, lead_id, cat_id).unwrap();
        let leads_after = db::get_leads_by_category(&conn, cat_id).unwrap();
        assert!(leads_after.is_empty());
        let lead_cats_after = db::get_categories_for_lead(&conn, lead_id).unwrap();
        assert!(lead_cats_after.is_empty());
    }

    #[test]
    fn test_outreach_history_logging() {
        let conn = db::init_db(":memory:").expect("Failed to init db");

        conn.execute(
            "INSERT INTO leads (name, company, status, last_activity, owner, email, phone) VALUES (?, ?, ?, ?, ?, ?, ?)",
            [
                "Jane Doe",
                "Company B",
                "Qualified",
                "2026-07-17T20:00:00Z",
                "Alex",
                "jane@example.com",
                "654321",
            ],
        ).unwrap();
        let lead_id = conn.last_insert_rowid() as i32;

        conn.execute(
            "INSERT INTO outreach_templates (name, content, created_at) VALUES (?, ?, ?)",
            ["Template A", "Hello", "2026-07-17T20:00:00Z"],
        )
        .unwrap();
        let template_id = conn.last_insert_rowid() as i32;

        let history = crate::models::OutreachHistory {
            id: None,
            lead_id,
            template_id,
            status: "SUCCESS".to_string(),
            error_message: None,
            sent_at: "2026-07-18T10:00:00Z".to_string(),
            session_imei: "imei-12345".to_string(),
        };

        db::log_outreach_attempt(&conn, &history).unwrap();

        let mut stmt = conn
            .prepare("SELECT lead_id, template_id, status, error_message, sent_at, session_imei FROM outreach_history")
            .unwrap();
        let mut rows = stmt.query([]).unwrap();
        let row = rows.next().unwrap().unwrap();
        assert_eq!(row.get::<_, i32>(0).unwrap(), lead_id);
        assert_eq!(row.get::<_, i32>(1).unwrap(), template_id);
        assert_eq!(row.get::<_, String>(2).unwrap(), "SUCCESS");
        assert_eq!(row.get::<_, Option<String>>(3).unwrap(), None);
        assert_eq!(row.get::<_, String>(4).unwrap(), "2026-07-18T10:00:00Z");
        assert_eq!(row.get::<_, String>(5).unwrap(), "imei-12345");
    }

    #[test]
    fn test_email_outreach_tables_exist() {
        let conn = db::init_db(":memory:").expect("Failed to init db");

        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap();
        let tables: Vec<String> = stmt
            .query_map([], |row: &rusqlite::Row| row.get::<_, String>(0))
            .unwrap()
            .map(|r: Result<String, rusqlite::Error>| r.unwrap())
            .collect();

        assert!(tables.contains(&"smtp_servers".to_string()));
        assert!(tables.contains(&"email_templates".to_string()));
        assert!(tables.contains(&"email_outreach_history".to_string()));
    }

    #[test]
    fn test_smtp_and_email_template_crud() {
        let conn = db::init_db(":memory:").expect("Failed to init db");

        // 1. Initially empty SMTP servers
        let smtp_servers = db::get_smtp_servers(&conn).unwrap();
        assert!(smtp_servers.is_empty());

        // 2. Create SMTP server
        let smtp = crate::models::SmtpServer {
            id: None,
            name: "Gmail SMTP".to_string(),
            host: "smtp.gmail.com".to_string(),
            port: 587,
            username: "user@gmail.com".to_string(),
            password: "password123".to_string(),
            from_email: "user@gmail.com".to_string(),
            from_name: "Sender".to_string(),
            security: "STARTTLS".to_string(),
            created_at: "".to_string(),
        };
        let smtp_id = db::save_smtp_server(&conn, &smtp).unwrap();
        assert!(smtp_id > 0);

        // 3. Get SMTP server by ID and list all
        let retrieved = db::get_smtp_server_by_id(&conn, smtp_id).unwrap().unwrap();
        assert_eq!(retrieved.name, "Gmail SMTP");
        assert_eq!(retrieved.host, "smtp.gmail.com");

        let all_smtp = db::get_smtp_servers(&conn).unwrap();
        assert_eq!(all_smtp.len(), 1);

        // 4. Create and CRUD Email Template
        let templates = db::get_email_templates(&conn).unwrap();
        assert!(templates.is_empty());

        let template_id =
            db::create_email_template(&conn, "Promo", "Special Offer", "Hello {{name}}").unwrap();
        assert!(template_id > 0);

        let templates = db::get_email_templates(&conn).unwrap();
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].subject, "Special Offer");

        db::update_email_template(
            &conn,
            template_id,
            "Promo Updated",
            "Updated Subject",
            "Hello {{name}} updated",
        )
        .unwrap();
        let templates = db::get_email_templates(&conn).unwrap();
        assert_eq!(templates[0].subject, "Updated Subject");

        // 5. Test logging email outreach attempt
        conn.execute(
            "INSERT INTO leads (name, company, status, last_activity, owner, email, phone) VALUES (?, ?, ?, ?, ?, ?, ?)",
            [
                "Test Lead",
                "Test Co",
                "New",
                "2026-07-24T12:00:00Z",
                "Owner",
                "lead@example.com",
                "123456789",
            ],
        ).unwrap();
        let lead_id = conn.last_insert_rowid() as i32;

        let history = crate::models::EmailOutreachHistory {
            id: None,
            lead_id,
            template_id,
            smtp_id,
            status: "SUCCESS".to_string(),
            error_message: None,
            sent_at: "2026-07-24T12:00:00Z".to_string(),
        };
        db::log_email_outreach_attempt(&conn, &history).unwrap();

        // 6. Delete SMTP & Template
        db::delete_email_template(&conn, template_id).unwrap();
        assert!(db::get_email_templates(&conn).unwrap().is_empty());

        db::delete_smtp_server(&conn, smtp_id).unwrap();
        assert!(db::get_smtp_servers(&conn).unwrap().is_empty());
        assert!(db::get_smtp_server_by_id(&conn, smtp_id).unwrap().is_none());
    }

    #[test]
    fn google_maps_job_schema_is_idempotent_and_enforces_keys() {
        let db_path = std::env::temp_dir().join(format!(
            "rustsale-google-maps-schema-{}.db",
            rand::random::<u64>()
        ));
        let db_path_str = db_path.to_str().unwrap();

        let conn = db::init_db(db_path_str).unwrap();
        drop(conn);
        let conn = db::init_db(db_path_str).unwrap();

        for table in [
            "google_maps_scrape_jobs",
            "google_maps_scrape_units",
            "google_maps_scrape_results",
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "missing table {table}");
        }

        let job_columns: Vec<String> = {
            let mut statement = conn
                .prepare("PRAGMA table_info(google_maps_scrape_jobs)")
                .unwrap();
            statement
                .query_map([], |row| row.get(1))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert!(job_columns.contains(&"profile_dir".to_string()));
        assert!(job_columns.contains(&"blocked_url".to_string()));

        let foreign_keys_enabled: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys_enabled, 1);

        let busy_timeout_ms: i64 = conn
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .unwrap();
        assert_eq!(busy_timeout_ms, 5_000);

        for index in [
            "idx_google_maps_jobs_status",
            "idx_google_maps_units_job_status",
            "idx_google_maps_results_job_import",
            "idx_leads_phone",
            "idx_leads_email_normalized",
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
                    [index],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "missing index {index}");
        }

        let orphan_unit = conn.execute(
            "INSERT INTO google_maps_scrape_units (
                id, job_id, unit_key, unit_type, payload_json, status, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "unit-orphan",
                "missing-job",
                "keyword:orphan",
                "keyword",
                "{}",
                "pending",
                "2026-07-26T00:00:00Z",
                "2026-07-26T00:00:00Z"
            ],
        );
        assert!(orphan_unit.is_err());

        drop(conn);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    #[test]
    fn google_maps_job_profile_columns_migrate_from_previous_schema() {
        let db_path = std::env::temp_dir().join(format!(
            "rustsale-google-maps-profile-migration-{}.db",
            rand::random::<u64>()
        ));
        let db_path_str = db_path.to_str().unwrap();
        let legacy = rusqlite::Connection::open(db_path_str).unwrap();
        legacy
            .execute(
                "CREATE TABLE google_maps_scrape_jobs (
                    id TEXT PRIMARY KEY,
                    mode TEXT NOT NULL,
                    keyword TEXT NOT NULL,
                    location TEXT,
                    bbox_json TEXT,
                    step_km REAL,
                    headless INTEGER NOT NULL CHECK (headless IN (0, 1)),
                    status TEXT NOT NULL,
                    stage TEXT NOT NULL,
                    total_units INTEGER NOT NULL DEFAULT 0 CHECK (total_units >= 0),
                    processed_units INTEGER NOT NULL DEFAULT 0 CHECK (processed_units >= 0),
                    failed_units INTEGER NOT NULL DEFAULT 0 CHECK (failed_units >= 0),
                    businesses_discovered INTEGER NOT NULL DEFAULT 0 CHECK (businesses_discovered >= 0),
                    businesses_qualified INTEGER NOT NULL DEFAULT 0 CHECK (businesses_qualified >= 0),
                    leads_created INTEGER NOT NULL DEFAULT 0 CHECK (leads_created >= 0),
                    last_error_kind TEXT,
                    last_error_message TEXT,
                    action_required TEXT,
                    event_sequence INTEGER NOT NULL DEFAULT 0 CHECK (event_sequence >= 0),
                    version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
                    run_generation INTEGER NOT NULL DEFAULT 0 CHECK (run_generation >= 0),
                    created_at TEXT NOT NULL,
                    started_at TEXT,
                    updated_at TEXT NOT NULL,
                    completed_at TEXT
                )",
                [],
            )
            .unwrap();
        drop(legacy);

        let connection = db::init_db(db_path_str).unwrap();
        drop(connection);
        let connection = db::init_db(db_path_str).unwrap();
        let columns: Vec<String> = {
            let mut statement = connection
                .prepare("PRAGMA table_info(google_maps_scrape_jobs)")
                .unwrap();
            statement
                .query_map([], |row| row.get(1))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };

        assert!(columns.contains(&"profile_dir".to_string()));
        assert!(columns.contains(&"blocked_url".to_string()));

        drop(connection);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("db-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("db-shm"));
    }

    #[test]
    fn google_maps_unit_key_is_unique_per_job() {
        let conn = db::init_db(":memory:").unwrap();
        conn.execute(
            "INSERT INTO google_maps_scrape_jobs (
                id, mode, keyword, headless, status, stage, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "job-1",
                "keyword",
                "coffee",
                1,
                "queued",
                "planning",
                "2026-07-26T00:00:00Z",
                "2026-07-26T00:00:00Z"
            ],
        )
        .unwrap();

        let insert_unit = |id: &str| {
            conn.execute(
                "INSERT INTO google_maps_scrape_units (
                    id, job_id, unit_key, unit_type, payload_json, status, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    id,
                    "job-1",
                    "keyword:coffee",
                    "keyword",
                    "{}",
                    "pending",
                    "2026-07-26T00:00:00Z",
                    "2026-07-26T00:00:00Z"
                ],
            )
        };

        insert_unit("unit-1").unwrap();
        assert!(insert_unit("unit-2").is_err());
    }

    #[test]
    fn google_maps_result_dedup_key_is_unique_per_job() {
        let conn = db::init_db(":memory:").unwrap();
        conn.execute(
            "INSERT INTO google_maps_scrape_jobs (
                id, mode, keyword, headless, status, stage, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "job-1",
                "keyword",
                "coffee",
                1,
                "queued",
                "planning",
                "2026-07-26T00:00:00Z",
                "2026-07-26T00:00:00Z"
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO google_maps_scrape_units (
                id, job_id, unit_key, unit_type, payload_json, status, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "unit-1",
                "job-1",
                "keyword:coffee",
                "keyword",
                "{}",
                "pending",
                "2026-07-26T00:00:00Z",
                "2026-07-26T00:00:00Z"
            ],
        )
        .unwrap();

        let insert_result = |id: &str| {
            conn.execute(
                "INSERT INTO google_maps_scrape_results (
                    id, job_id, source_unit_id, dedup_key, name, detail_status,
                    enrichment_status, import_status, first_seen_at, last_seen_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![
                    id,
                    "job-1",
                    "unit-1",
                    "place:abc",
                    "Coffee Shop",
                    "pending",
                    "pending",
                    "pending",
                    "2026-07-26T00:00:00Z",
                    "2026-07-26T00:00:00Z"
                ],
            )
        };

        insert_result("result-1").unwrap();
        assert!(insert_result("result-2").is_err());
    }

    #[test]
    fn monetary_metadata_initialization_preserves_existing_values_without_corruption() {
        let path = std::env::temp_dir().join(format!(
            "rustsale-currency-metadata-init-{}.db",
            rand::random::<u64>()
        ));
        let path_str = path.to_string_lossy().to_string();

        // 1. Create a database without app_metadata containing mixed monetary values
        {
            let raw_conn = rusqlite::Connection::open(&path_str).unwrap();
            raw_conn
                .execute(
                    "CREATE TABLE leads (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        name TEXT NOT NULL,
                        company TEXT NOT NULL,
                        status TEXT NOT NULL,
                        value REAL NOT NULL DEFAULT 0.0,
                        last_activity TEXT NOT NULL,
                        owner TEXT NOT NULL,
                        email TEXT NOT NULL,
                        phone TEXT NOT NULL,
                        about TEXT,
                        address TEXT,
                        has_zalo INTEGER DEFAULT NULL,
                        zalo_reached INTEGER NOT NULL DEFAULT 0,
                        zalo_uid TEXT,
                        zalo_status TEXT DEFAULT 'UNTESTED',
                        relationship_status TEXT DEFAULT 'UNKNOWN',
                        last_lookup_attempt TEXT
                    )",
                    [],
                )
                .unwrap();
            raw_conn
                .execute(
                    "CREATE TABLE deals (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        lead_id INTEGER NOT NULL,
                        title TEXT NOT NULL,
                        value REAL NOT NULL DEFAULT 0.0,
                        stage TEXT NOT NULL,
                        created_at TEXT NOT NULL
                    )",
                    [],
                )
                .unwrap();

            // Insert distinct monetary amounts including small values (500k VND) that must NOT be scaled
            raw_conn
                .execute(
                    "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone)
                     VALUES ('Small Value Lead', 'Corp A', 'New', 500000.0, '2026-07-01T00:00:00Z', 'Owner', 'a@test.com', '111')",
                    [],
                )
                .unwrap();
            raw_conn
                .execute(
                    "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone)
                     VALUES ('Large Value Lead', 'Corp B', 'Qualified', 45000000.0, '2026-07-01T00:00:00Z', 'Owner', 'b@test.com', '222')",
                    [],
                )
                .unwrap();
            raw_conn
                .execute(
                    "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone)
                     VALUES ('Zero Value Lead', 'Corp C', 'New', 0.0, '2026-07-01T00:00:00Z', 'Owner', 'c@test.com', '333')",
                    [],
                )
                .unwrap();

            raw_conn
                .execute(
                    "INSERT INTO deals (lead_id, title, value, stage, created_at)
                     VALUES (1, 'Deal 750k', 750000.0, 'Prospecting', '2026-07-01T00:00:00Z')",
                    [],
                )
                .unwrap();
            raw_conn
                .execute(
                    "INSERT INTO deals (lead_id, title, value, stage, created_at)
                     VALUES (2, 'Deal 15M', 15000000.0, 'Proposal', '2026-07-01T00:00:00Z')",
                    [],
                )
                .unwrap();
        }

        // 2. Open via db::init_db to initialize schema and metadata
        let conn = db::init_db(&path_str).expect("init_db should succeed on pre-existing database");

        // 3. Verify app_metadata has currency_semantics = 'MULTI_CURRENCY_v1'
        let version: String = conn
            .query_row(
                "SELECT value FROM app_metadata WHERE key = 'currency_semantics'",
                [],
                |row| row.get(0),
            )
            .expect("app_metadata currency_semantics should be set");
        assert_eq!(version, "MULTI_CURRENCY_v1");

        // 4. Verify all monetary values were preserved exactly without distortion or heuristic scaling
        let lead1_val: f64 = conn
            .query_row("SELECT value FROM leads WHERE id = 1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(lead1_val, 500000.0, "500k VND amount must remain 500,000.0");
        let lead1_curr: String = conn
            .query_row("SELECT currency FROM leads WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(lead1_curr, "VND", "Migrated lead should default to VND");

        let lead2_val: f64 = conn
            .query_row("SELECT value FROM leads WHERE id = 2", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            lead2_val, 45000000.0,
            "45M VND amount must remain 45,000,000.0"
        );

        let lead3_val: f64 = conn
            .query_row("SELECT value FROM leads WHERE id = 3", [], |row| row.get(0))
            .unwrap();
        assert_eq!(lead3_val, 0.0, "0.0 VND amount must remain 0.0");

        let deal1_val: f64 = conn
            .query_row("SELECT value FROM deals WHERE id = 1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(deal1_val, 750000.0, "750k VND deal must remain 750,000.0");
        let deal1_curr: String = conn
            .query_row("SELECT currency FROM deals WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(deal1_curr, "VND", "Migrated deal should default to VND");

        let deal2_val: f64 = conn
            .query_row("SELECT value FROM deals WHERE id = 2", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            deal2_val, 15000000.0,
            "15M VND deal must remain 15,000,000.0"
        );

        // 5. Test SQLite triggers: inserting or updating invalid currency MUST fail
        let bad_deal_insert = conn.execute(
            "INSERT INTO deals (lead_id, title, value, stage, created_at, currency)
             VALUES (1, 'Bad Deal', 100.0, 'Lead', '2026-07-01T00:00:00Z', 'EUR')",
            [],
        );
        assert!(
            bad_deal_insert.is_err(),
            "Trigger must reject invalid currency EUR on deal insert"
        );

        let bad_lead_insert = conn.execute(
            "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, currency)
             VALUES ('Bad Lead', 'Co', 'New', 100.0, '2026-07-01T00:00:00Z', 'Owner', 'a@b.com', '123', 'XYZ')",
            [],
        );
        assert!(
            bad_lead_insert.is_err(),
            "Trigger must reject invalid currency XYZ on lead insert"
        );

        let bad_deal_update = conn.execute("UPDATE deals SET currency = 'JPY' WHERE id = 1", []);
        assert!(
            bad_deal_update.is_err(),
            "Trigger must reject invalid currency JPY on deal update"
        );

        // 6. Verify idempotency: re-running init_db preserves values and metadata
        drop(conn);
        let conn_again = db::init_db(&path_str).expect("re-opening db succeeds");
        let lead1_again: f64 = conn_again
            .query_row("SELECT value FROM leads WHERE id = 1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(lead1_again, 500000.0);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_unified_campaign_and_steps_crud() {
        let conn = db::init_db(":memory:").expect("Failed to init db");

        // 1. Create campaign
        let camp_id = db::create_campaign(
            &conn,
            "SaaS Summer Outreach",
            Some("Multi-channel outreach to tech CEOs"),
            "omnichannel",
            "DRAFT",
            Some(r#"{"status": "New", "has_zalo": 1}"#),
            Some(r#"{"days": [1,2,3,4,5], "start_time": "09:00", "end_time": "17:00"}"#),
        )
        .expect("create_campaign failed");

        assert!(camp_id > 0);

        // 2. Read campaign
        let camp = db::get_campaign(&conn, camp_id)
            .expect("get_campaign failed")
            .expect("campaign not found");
        assert_eq!(camp.name, "SaaS Summer Outreach");
        assert_eq!(camp.channel_type, "omnichannel");
        assert_eq!(camp.status, "DRAFT");

        // 3. Update campaign
        db::update_campaign(
            &conn,
            camp_id,
            "SaaS Summer Outreach v2",
            Some("Updated description"),
            "zalo",
            "ACTIVE",
            Some(r#"{"status": "Qualified"}"#),
            None,
        )
        .expect("update_campaign failed");

        let updated_camp = db::get_campaign(&conn, camp_id)
            .expect("get_campaign failed")
            .unwrap();
        assert_eq!(updated_camp.name, "SaaS Summer Outreach v2");
        assert_eq!(updated_camp.status, "ACTIVE");
        assert_eq!(updated_camp.channel_type, "zalo");

        // 4. Create campaign steps
        let step1_id = db::create_campaign_step(
            &conn,
            camp_id,
            1,
            "zalo",
            0,
            "ALWAYS",
            "Hello {name}, welcome to RustSale!",
            Some("Keep it friendly and brief."),
        )
        .expect("create step 1 failed");

        let step2_id = db::create_campaign_step(
            &conn,
            camp_id,
            2,
            "whatsapp",
            2880, // 2 days
            "IF_NO_REPLY",
            "Hi {name}, following up on our previous note.",
            None,
        )
        .expect("create step 2 failed");

        let steps = db::get_campaign_steps(&conn, camp_id).expect("get_campaign_steps failed");
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].id, Some(step1_id));
        assert_eq!(steps[0].step_order, 1);
        assert_eq!(steps[1].id, Some(step2_id));
        assert_eq!(steps[1].step_order, 2);
        assert_eq!(steps[1].delay_minutes, 2880);

        // 5. Update step
        db::update_campaign_step(
            &conn,
            step1_id,
            1,
            "zalo",
            10,
            "ALWAYS",
            "Updated template for {name}",
            None,
        )
        .expect("update step 1 failed");

        let step1 = db::get_campaign_step(&conn, step1_id)
            .expect("get step 1 failed")
            .unwrap();
        assert_eq!(step1.template_content, "Updated template for {name}");
        assert_eq!(step1.delay_minutes, 10);

        // 6. Delete step & campaign cascade
        db::delete_campaign_step(&conn, step2_id).expect("delete step 2 failed");
        let steps_after_delete = db::get_campaign_steps(&conn, camp_id).unwrap();
        assert_eq!(steps_after_delete.len(), 1);

        db::delete_campaign(&conn, camp_id).expect("delete campaign failed");
        assert!(db::get_campaign(&conn, camp_id).unwrap().is_none());
        assert_eq!(db::get_campaign_steps(&conn, camp_id).unwrap().len(), 0);
    }

    #[test]
    fn test_campaign_executions_and_due_query() {
        let conn = db::init_db(":memory:").expect("Failed to init db");

        // Insert lead
        conn.execute(
            "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone)
             VALUES ('John Doe', 'TestCo', 'New', 1000.0, '2026-09-01T00:00:00Z', 'Alex', 'john@test.com', '0911223344')",
            [],
        )
        .unwrap();
        let lead_id = conn.last_insert_rowid() as i32;

        let camp_id = db::create_campaign(
            &conn,
            "Follow-up Wave 1",
            None,
            "zalo",
            "ACTIVE",
            None,
            None,
        )
        .unwrap();

        let step_id = db::create_campaign_step(
            &conn,
            camp_id,
            1,
            "zalo",
            0,
            "ALWAYS",
            "Initial touchpoint",
            None,
        )
        .unwrap();

        // Create executions: one past due, one in the future
        let exec1_id = db::create_campaign_execution(
            &conn,
            camp_id,
            step_id,
            lead_id,
            Some("0911223344"),
            None,
            "2026-09-01T10:00:00Z",
        )
        .expect("create exec1 failed");

        let _exec2_id = db::create_campaign_execution(
            &conn,
            camp_id,
            step_id,
            lead_id,
            Some("0911223344"),
            None,
            "2026-09-03T10:00:00Z",
        )
        .expect("create exec2 failed");

        // Query due executions at 2026-09-02T00:00:00Z
        let due = db::get_due_campaign_executions(&conn, "2026-09-02T00:00:00Z", 10)
            .expect("get_due_campaign_executions failed");
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, Some(exec1_id));

        // Update status to EXECUTED
        db::update_campaign_execution_status(
            &conn,
            exec1_id,
            "COMPLETED",
            None,
            Some("2026-09-02T00:01:00Z"),
        )
        .expect("update status failed");

        let due_after = db::get_due_campaign_executions(&conn, "2026-09-02T00:00:00Z", 10).unwrap();
        assert_eq!(due_after.len(), 0);
    }

    #[test]
    fn test_lead_journey_state_crud() {
        let conn = db::init_db(":memory:").expect("Failed to init db");

        conn.execute(
            "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone)
             VALUES ('Jane Smith', 'Innovate', 'Discovery', 5000.0, '2026-09-01T00:00:00Z', 'Alex', 'jane@innovate.com', '0922334455')",
            [],
        )
        .unwrap();
        let lead_id = conn.last_insert_rowid() as i32;

        // 1. Get or create initial state
        let initial_state = db::get_or_create_lead_journey_state(&conn, lead_id)
            .expect("get_or_create_lead_journey_state failed");
        assert_eq!(initial_state.lead_id, lead_id);
        assert_eq!(initial_state.current_stage, "DISCOVERY");
        assert!(initial_state.active_campaign_id.is_none());

        // Create campaign for valid foreign key reference
        let camp_id = db::create_campaign(
            &conn,
            "Onboarding Wave",
            None,
            "omnichannel",
            "ACTIVE",
            None,
            None,
        )
        .expect("create_campaign failed");

        // 2. Update journey state
        db::update_lead_journey_state(
            &conn,
            lead_id,
            "QUALIFIED",
            Some(camp_id),
            Some("2026-09-02T08:00:00Z"),
            Some("2026-09-02T08:30:00Z"),
            Some("AskingPrice"),
            Some(0.92),
            Some("Customer expressed high buying intent"),
        )
        .expect("update_lead_journey_state failed");

        let updated_state = db::get_lead_journey_state(&conn, lead_id)
            .expect("get_lead_journey_state failed")
            .unwrap();
        assert_eq!(updated_state.current_stage, "QUALIFIED");
        assert_eq!(updated_state.active_campaign_id, Some(camp_id));
        assert_eq!(updated_state.ai_intent_tag.as_deref(), Some("AskingPrice"));
        assert_eq!(updated_state.ai_qualification_score, Some(0.92));
    }

    #[test]
    fn test_ai_evidence_logs_and_proposals_crud() {
        let conn = db::init_db(":memory:").expect("Failed to init db");

        conn.execute(
            "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone)
             VALUES ('Mark Wood', 'TechCorp', 'New', 10000.0, '2026-09-01T00:00:00Z', 'Alex', 'mark@techcorp.com', '0933445566')",
            [],
        )
        .unwrap();
        let lead_id = conn.last_insert_rowid() as i32;

        // 1. Insert AI evidence log
        let log_id = db::insert_ai_evidence_log(
            &conn,
            Some(lead_id),
            Some(1001),
            "INTENT_CLASSIFICATION",
            "PRICING_CATALOGUE",
            r#"{"matched_plan": "Enterprise", "price_vnd": 45000000}"#,
            0.98,
            "OFFER_STANDARD_DISCOUNT",
        )
        .expect("insert_ai_evidence_log failed");
        assert!(log_id > 0);

        let logs = db::get_ai_evidence_logs_by_lead(&conn, lead_id)
            .expect("get_ai_evidence_logs_by_lead failed");
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].action_type, "INTENT_CLASSIFICATION");
        assert_eq!(logs[0].confidence_score, 0.98);

        // 2. Create AI action proposal
        let prop_id = db::create_ai_action_proposal(
            &conn,
            Some(lead_id),
            Some(1001),
            "zalo",
            "Gói Enterprise bên em có giá 45 triệu / năm với đầy đủ tính năng CRM & Outreach.",
            "Customer asked for Enterprise pricing in message #1001",
        )
        .expect("create_ai_action_proposal failed");

        let pending_props = db::get_ai_action_proposals(&conn, Some("PENDING"))
            .expect("get_ai_action_proposals failed");
        assert_eq!(pending_props.len(), 1);
        assert_eq!(pending_props[0].id, Some(prop_id));
        assert_eq!(pending_props[0].status, "PENDING");

        // 3. Update proposal status
        db::update_ai_action_proposal_status(&conn, prop_id, "APPROVED", Some("SalesManager"))
            .expect("update_ai_action_proposal_status failed");

        let prop = db::get_ai_action_proposal(&conn, prop_id)
            .expect("get_ai_action_proposal failed")
            .unwrap();
        assert_eq!(prop.status, "APPROVED");
        assert_eq!(prop.reviewed_by.as_deref(), Some("SalesManager"));
        assert!(prop.reviewed_at.is_some());
    }
}

