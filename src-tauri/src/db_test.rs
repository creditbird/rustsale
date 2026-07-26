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

        db::remove_lead_from_category(&conn, lead_id, cat_id).unwrap();
        let leads_after = db::get_leads_by_category(&conn, cat_id).unwrap();
        assert!(leads_after.is_empty());
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
}
