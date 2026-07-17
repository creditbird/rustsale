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
            .query_row("SELECT address FROM leads WHERE id = 1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(address1, Some("123 Main St, New York".to_string()));

        let has_zalo1: Option<i32> = conn
            .query_row("SELECT has_zalo FROM leads WHERE id = 1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(has_zalo1, Some(1));

        let zalo_reached1: i32 = conn
            .query_row("SELECT zalo_reached FROM leads WHERE id = 1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(zalo_reached1, 0);

        // Verify new fields on Jordan Smith (id = 2)
        let address2: Option<String> = conn
            .query_row("SELECT address FROM leads WHERE id = 2", [], |row| row.get(0))
            .unwrap();
        assert_eq!(address2, Some("456 Oak Ave, Boston".to_string()));

        let has_zalo2: Option<i32> = conn
            .query_row("SELECT has_zalo FROM leads WHERE id = 2", [], |row| row.get(0))
            .unwrap();
        assert_eq!(has_zalo2, None); // NULL in database

        let zalo_reached2: i32 = conn
            .query_row("SELECT zalo_reached FROM leads WHERE id = 2", [], |row| row.get(0))
            .unwrap();
        assert_eq!(zalo_reached2, 0);

        // Verify lele (id = 3)
        let lead3_name: String = conn
            .query_row("SELECT name FROM leads WHERE id = 3", [], |row| row.get(0))
            .unwrap();
        assert_eq!(lead3_name, "lele");

        let address3: Option<String> = conn
            .query_row("SELECT address FROM leads WHERE id = 3", [], |row| row.get(0))
            .unwrap();
        assert_eq!(address3, Some("789 Pine Rd, Seattle".to_string()));

        let has_zalo3: Option<i32> = conn
            .query_row("SELECT has_zalo FROM leads WHERE id = 3", [], |row| row.get(0))
            .unwrap();
        assert_eq!(has_zalo3, Some(1));

        let zalo_reached3: i32 = conn
            .query_row("SELECT zalo_reached FROM leads WHERE id = 3", [], |row| row.get(0))
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
        let id1 = db::create_outreach_template(&conn, "Template One", "Hello {name}").unwrap();
        assert!(id1 > 0);

        let id2 = db::create_outreach_template(&conn, "Template Two", "Hi {name}").unwrap();
        assert!(id2 > 0);

        // 3. Get all templates
        let templates = db::get_outreach_templates(&conn).unwrap();
        assert_eq!(templates.len(), 2);
        assert_eq!(templates[0].name, "Template One");
        assert_eq!(templates[0].content, "Hello {name}");

        // 4. Update template
        db::update_outreach_template(&conn, id1, "Template One Updated", "Hello {name}!").unwrap();
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
}
