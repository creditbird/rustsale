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
}
