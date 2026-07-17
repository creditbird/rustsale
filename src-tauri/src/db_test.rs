#[cfg(test)]
mod tests {
    use super::super::db;

    #[test]
    fn test_db_init_and_tables() {
        let conn = db::init_db(":memory:").expect("Failed to init db");
        
        let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table'").unwrap();
        let tables: Vec<String> = stmt.query_map([], |row: &rusqlite::Row| row.get::<_, String>(0)).unwrap()
            .map(|r: Result<String, rusqlite::Error>| r.unwrap()).collect();
            
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
        let lead_count: i64 = conn.query_row("SELECT COUNT(*) FROM leads", [], |row| row.get(0)).unwrap();
        assert_eq!(lead_count, 2);

        let lead_name: String = conn.query_row("SELECT name FROM leads WHERE id = 1", [], |row| row.get(0)).unwrap();
        assert_eq!(lead_name, "Sarah Harrington");

        // Verify deals
        let deal_count: i64 = conn.query_row("SELECT COUNT(*) FROM deals", [], |row| row.get(0)).unwrap();
        assert_eq!(deal_count, 2);

        // Verify activities
        let activity_count: i64 = conn.query_row("SELECT COUNT(*) FROM activities", [], |row| row.get(0)).unwrap();
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
        let imei: String = conn.query_row(
            "SELECT imei FROM zalo_sessions WHERE language = 'vi'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(imei, "123456789012345");

        // Test inserting an outreach template
        conn.execute(
            "INSERT INTO outreach_templates (name, content, created_at) VALUES (?, ?, ?)",
            [
                "Welcome Template",
                "Hello, {{name}}!",
                "2026-07-17T20:00:00Z",
            ],
        ).unwrap();

        // Verify template retrieval
        let content: String = conn.query_row(
            "SELECT content FROM outreach_templates WHERE name = 'Welcome Template'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(content, "Hello, {{name}}!");
    }
}
