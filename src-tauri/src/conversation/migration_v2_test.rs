#[cfg(test)]
mod tests {
    use crate::db::init_db;
    use super::super::migration_v2::{apply_migration_v2, apply_migration_v3};
    use rusqlite::Connection;

    fn temp_db_connection() -> (Connection, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "rustsale-migration-v2-test-{}.db",
            rand::random::<u64>()
        ));
        let conn = init_db(path.to_str().unwrap()).unwrap();
        (conn, path)
    }

    #[test]
    fn test_fresh_database_to_v3() {
        let (conn, path) = temp_db_connection();
        let _guard = defer_remove(path);

        let v: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, 3);

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='channel_sync_dead_letters'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_multi_page_psid_isolation() {
        let (conn, path) = temp_db_connection();
        let _guard = defer_remove(path);

        // 1. Create two Facebook Page channel accounts
        conn.execute(
            "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at)
             VALUES (1, 'messenger', 'PAGE_1', '2026-01-01', '2026-01-01')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at)
             VALUES (2, 'messenger', 'PAGE_2', '2026-01-01', '2026-01-01')",
            [],
        ).unwrap();

        // 2. Insert same PSID for Page 1 and Page 2 -> Both MUST succeed
        conn.execute(
            "INSERT INTO channel_identities (channel, channel_account_id, identity_namespace, external_id, created_at, updated_at)
             VALUES ('messenger', 1, 'psid', 'PSID_COMMON_123', '2026-01-01', '2026-01-01')",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO channel_identities (channel, channel_account_id, identity_namespace, external_id, created_at, updated_at)
             VALUES ('messenger', 2, 'psid', 'PSID_COMMON_123', '2026-01-01', '2026-01-01')",
            [],
        ).unwrap();

        // 3. Duplicate PSID under Page 1 -> MUST FAIL
        let duplicate_res = conn.execute(
            "INSERT INTO channel_identities (channel, channel_account_id, identity_namespace, external_id, created_at, updated_at)
             VALUES ('messenger', 1, 'psid', 'PSID_COMMON_123', '2026-01-01', '2026-01-01')",
            [],
        );
        assert!(duplicate_res.is_err(), "Duplicate PSID under the same Page must violate unique constraint");
    }

    #[test]
    fn test_existing_v2_to_v3_upgrade_preserves_data() {
        let conn = Connection::open_in_memory().unwrap();

        // Base V1 schema fixture
        conn.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS leads (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS channel_accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel TEXT NOT NULL CHECK (channel IN ('zalo', 'whatsapp', 'messenger', 'email')),
                provider_account_key TEXT NOT NULL,
                display_name TEXT,
                avatar_url TEXT,
                enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
                connection_status TEXT NOT NULL DEFAULT 'disconnected',
                last_connected_at TEXT,
                last_error TEXT,
                capabilities_json TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(channel, provider_account_key)
            );
            CREATE TABLE IF NOT EXISTS channel_identities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                lead_id INTEGER,
                channel TEXT NOT NULL CHECK (channel IN ('zalo', 'whatsapp', 'messenger', 'email')),
                identity_namespace TEXT NOT NULL,
                external_id TEXT NOT NULL,
                normalized_address TEXT,
                display_name TEXT,
                avatar_url TEXT,
                profile_synced_at TEXT,
                profile_sync_error TEXT,
                profile_sync_attempts INTEGER NOT NULL DEFAULT 0,
                next_profile_retry_at TEXT,
                link_source TEXT NOT NULL DEFAULT 'unlinked',
                link_confidence REAL,
                verified_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(channel, identity_namespace, external_id),
                FOREIGN KEY(lead_id) REFERENCES leads(id) ON DELETE SET NULL
            );
            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                conversation_id INTEGER NOT NULL,
                text_content TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS message_attachments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id INTEGER NOT NULL,
                remote_url TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
        "#).unwrap();

        // 1. Setup V2 database
        apply_migration_v2(&conn).unwrap();
        let v2: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v2, 2);

        // 2. Populate test data in V2
        conn.execute(
            "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at)
             VALUES (1, 'messenger', 'PAGE_1', '2026-01-01', '2026-01-01')",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO channel_identities (id, channel, channel_account_id, identity_namespace, external_id, created_at, updated_at)
             VALUES (1, 'messenger', 1, 'psid', 'USER_1', '2026-01-01', '2026-01-01')",
            [],
        ).unwrap();

        // 3. Upgrade from V2 to V3
        apply_migration_v3(&conn).unwrap();
        let v3: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v3, 3);

        // Verify preserved identity data
        let identity_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM channel_identities WHERE external_id = 'USER_1' AND channel_account_id = 1",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(identity_count, 1);

        // 4. Idempotency check
        apply_migration_v3(&conn).unwrap();
        let v3_repeat: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v3_repeat, 3);
    }

    struct DeferRemove(std::path::PathBuf);
    impl Drop for DeferRemove {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    fn defer_remove(path: std::path::PathBuf) -> DeferRemove {
        DeferRemove(path)
    }
}
