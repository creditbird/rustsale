#[cfg(test)]
mod tests {
    use super::super::migration_v2::{apply_migration_v2, apply_migration_v3, apply_migration_v4};
    use crate::db::{init_db, open_db_connection};
    use rusqlite::Connection;

    #[test]
    fn test_fresh_database_to_v3() {
        let path = std::env::temp_dir().join(format!(
            "rustsale-migration-fresh-{}.db",
            rand::random::<u64>()
        ));
        {
            let conn = init_db(path.to_str().unwrap()).unwrap();
            let v: u32 = conn
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .unwrap();
            assert_eq!(v, 4);

            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='channel_sync_dead_letters'",
                [],
                |r| r.get(0),
            ).unwrap();
            assert_eq!(count, 1);
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_multi_page_psid_isolation() {
        let path = std::env::temp_dir().join(format!(
            "rustsale-migration-isolation-{}.db",
            rand::random::<u64>()
        ));
        {
            let conn = init_db(path.to_str().unwrap()).unwrap();

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
            assert!(
                duplicate_res.is_err(),
                "Duplicate PSID under the same Page must violate unique constraint"
            );
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_populated_production_v1_to_v3_migration_preserves_foreign_keys_and_dependents() {
        let path = std::env::temp_dir().join(format!(
            "rustsale-migration-populated-{}.db",
            rand::random::<u64>()
        ));
        {
            let mut conn = open_db_connection(path.to_str().unwrap()).unwrap();

            // 1. Build canonical historical V1 production schema
            conn.execute_batch(r#"
                PRAGMA foreign_keys = ON;

                CREATE TABLE leads (
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
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE channel_accounts (
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

                CREATE TABLE channel_identities (
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

                CREATE TABLE channel_conversations (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    channel_account_id INTEGER NOT NULL,
                    external_thread_id TEXT NOT NULL,
                    kind TEXT NOT NULL CHECK (kind IN ('direct', 'group', 'email_thread')),
                    title TEXT,
                    avatar_url TEXT,
                    status TEXT NOT NULL DEFAULT 'active',
                    last_message_at TEXT,
                    last_synced_at TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    UNIQUE(channel_account_id, external_thread_id, kind),
                    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
                );

                CREATE TABLE conversation_participants (
                    conversation_id INTEGER NOT NULL,
                    identity_id INTEGER NOT NULL,
                    participant_role TEXT NOT NULL DEFAULT 'member',
                    joined_at TEXT,
                    left_at TEXT,
                    PRIMARY KEY(conversation_id, identity_id),
                    FOREIGN KEY(conversation_id) REFERENCES channel_conversations(id) ON DELETE CASCADE,
                    FOREIGN KEY(identity_id) REFERENCES channel_identities(id) ON DELETE CASCADE
                );

                CREATE TABLE messages (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    conversation_id INTEGER NOT NULL,
                    sender_identity_id INTEGER,
                    direction TEXT NOT NULL CHECK (direction IN ('inbound', 'outbound', 'system')),
                    provider_message_id TEXT,
                    provider_client_message_id TEXT,
                    local_request_id TEXT,
                    content_type TEXT NOT NULL,
                    text_content TEXT,
                    content_json TEXT NOT NULL DEFAULT '{}',
                    raw_payload_json TEXT,
                    provider_timestamp TEXT,
                    ingested_at TEXT NOT NULL,
                    sent_at TEXT,
                    status TEXT NOT NULL DEFAULT 'received' CHECK (status IN ('draft', 'queued', 'sending', 'sent', 'delivered', 'read', 'failed', 'received')),
                    removed_at TEXT,
                    error_message TEXT,
                    reply_to_message_id INTEGER,
                    reply_sender_identity_id INTEGER,
                    send_batch_id TEXT,
                    batch_index INTEGER,
                    FOREIGN KEY(conversation_id) REFERENCES channel_conversations(id) ON DELETE CASCADE,
                    FOREIGN KEY(sender_identity_id) REFERENCES channel_identities(id) ON DELETE SET NULL,
                    FOREIGN KEY(reply_to_message_id) REFERENCES messages(id) ON DELETE SET NULL,
                    FOREIGN KEY(reply_sender_identity_id) REFERENCES channel_identities(id) ON DELETE SET NULL
                );

                CREATE TABLE message_attachments (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    message_id INTEGER NOT NULL,
                    provider_attachment_id TEXT,
                    attachment_type TEXT NOT NULL DEFAULT 'file',
                    file_name TEXT,
                    mime_type TEXT,
                    size_bytes INTEGER,
                    remote_url TEXT,
                    local_path TEXT,
                    thumbnail_url TEXT,
                    metadata_json TEXT NOT NULL DEFAULT '{}',
                    created_at TEXT NOT NULL,
                    FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE
                );
            "#).unwrap();

            // 2. Populate populated production rows across all tables
            conn.execute_batch(r#"
                INSERT INTO leads (id, name, company, status, value, last_activity, owner, email, phone, created_at, updated_at)
                VALUES (101, 'Alice Corp', 'AliceCo', 'Active', 1000.0, '2026-01-01', 'Admin', 'alice@test.com', '123', '2026-01-01', '2026-01-01');

                INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, created_at, updated_at)
                VALUES (1, 'zalo', 'ZALO_ACC_1', 'Zalo Shop', '2026-01-01', '2026-01-01');

                INSERT INTO channel_identities (id, lead_id, channel, identity_namespace, external_id, display_name, created_at, updated_at)
                VALUES (501, 101, 'zalo', 'zalo_uid', 'USER_ALICE', 'Alice Inbound', '2026-01-01', '2026-01-01');

                INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, title, created_at, updated_at)
                VALUES (301, 1, 'THREAD_1', 'direct', 'Chat with Alice', '2026-01-01', '2026-01-01');

                INSERT INTO conversation_participants (conversation_id, identity_id, participant_role)
                VALUES (301, 501, 'member');

                INSERT INTO messages (id, conversation_id, sender_identity_id, direction, content_type, text_content, ingested_at, status)
                VALUES (701, 301, 501, 'inbound', 'text', 'Hello world', '2026-01-01', 'received');

                -- Add duplicate attachments for message 701 with conflicting partial metadata
                INSERT INTO message_attachments (id, message_id, provider_attachment_id, file_name, mime_type, remote_url, metadata_json, created_at)
                VALUES (901, 701, 'META_ATT_1', 'photo.jpg', 'image/jpeg', 'https://cdn.example.com/pic1.jpg', '{"width": 800}', '2026-01-01');

                INSERT INTO message_attachments (id, message_id, provider_attachment_id, file_name, mime_type, remote_url, local_path, thumbnail_url, metadata_json, created_at)
                VALUES (902, 701, NULL, NULL, NULL, 'https://cdn.example.com/pic1.jpg', '/spool/local.jpg', 'https://cdn.example.com/thumb.jpg', '{"height": 600}', '2026-01-01');

                -- Add duplicate attachments for message 701 with colliding provider_attachment_id without remote_url
                INSERT INTO message_attachments (id, message_id, provider_attachment_id, file_name, mime_type, remote_url, metadata_json, created_at)
                VALUES (903, 701, 'PROVIDER_COLLISION_1', 'photo_a.jpg', 'image/jpeg', NULL, '{"a": 1}', '2026-01-01');

                INSERT INTO message_attachments (id, message_id, provider_attachment_id, file_name, mime_type, remote_url, metadata_json, created_at)
                VALUES (904, 701, 'PROVIDER_COLLISION_1', 'photo_b.jpg', 'image/jpeg', NULL, '{"b": 2}', '2026-01-01');
            "#).unwrap();

            // 3. Execute migration under outer transaction (exactly matching db.rs::init_db production startup path)
            conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
            {
                let tx = conn.transaction().unwrap();
                crate::conversation::migration::migrate(&tx).unwrap();
                tx.commit().unwrap();
            }
            conn.pragma_update(None, "foreign_keys", "ON").unwrap();

            // 4. Verify user_version is 4
            let v: u32 = conn
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .unwrap();
            assert_eq!(v, 4);

            // 5. Verify 0 foreign key violations
            let mut fk_stmt = conn.prepare("PRAGMA foreign_key_check").unwrap();
            let fk_violations: Vec<String> = fk_stmt
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();
            assert!(
                fk_violations.is_empty(),
                "Foreign key violations detected after migration: {:?}",
                fk_violations
            );

            // 6. Verify channel_identities row preserved with correct ID and columns
            let identity_lead_id: Option<i64> = conn
                .query_row(
                    "SELECT lead_id FROM channel_identities WHERE id = 501",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(identity_lead_id, Some(101));

            // 7. Verify conversation_participants still references identity 501 without being cascaded/nulled
            let participant_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM conversation_participants WHERE conversation_id = 301 AND identity_id = 501",
                [],
                |r| r.get(0),
            ).unwrap();
            assert_eq!(
                participant_count, 1,
                "Participant reference was lost during migration"
            );

            // 8. Verify messages sender_identity_id preserved
            let sender_id: Option<i64> = conn
                .query_row(
                    "SELECT sender_identity_id FROM messages WHERE id = 701",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(sender_id, Some(501));

            // 9. Verify lossless attachment deduplication: exactly 1 attachment record with merged metadata
            let (att_id, provider_att_id, file_name, local_path, thumb_url, meta_json): (i64, Option<String>, Option<String>, Option<String>, Option<String>, String) = conn.query_row(
                "SELECT id, provider_attachment_id, file_name, local_path, thumbnail_url, metadata_json 
                 FROM message_attachments WHERE message_id = 701 AND remote_url = 'https://cdn.example.com/pic1.jpg'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            ).unwrap();
            assert_eq!(att_id, 901);
            assert_eq!(provider_att_id, Some("META_ATT_1".to_string()));
            assert_eq!(file_name, Some("photo.jpg".to_string()));
            assert_eq!(local_path, Some("/spool/local.jpg".to_string()));
            assert_eq!(
                thumb_url,
                Some("https://cdn.example.com/thumb.jpg".to_string())
            );
            assert!(meta_json.contains("\"width\":800") || meta_json.contains("\"width\": 800"));
            assert!(meta_json.contains("\"height\":600") || meta_json.contains("\"height\": 600"));

            // 10. Verify unique index prevents future duplicate remote_url
            let dup_insert = conn.execute(
                "INSERT INTO message_attachments (message_id, remote_url, created_at)
                 VALUES (701, 'https://cdn.example.com/pic1.jpg', '2026-01-01')",
                [],
            );
            assert!(
                dup_insert.is_err(),
                "Duplicate attachment remote_url must violate unique index"
            );

            // 11. Verify provider_attachment_id collision deduplication: exactly 1 attachment record with merged metadata
            let (p_att_id, p_prov_id, p_file_name, p_meta): (i64, Option<String>, Option<String>, String) = conn.query_row(
                "SELECT id, provider_attachment_id, file_name, metadata_json 
                 FROM message_attachments WHERE message_id = 701 AND provider_attachment_id = 'PROVIDER_COLLISION_1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            ).unwrap();
            assert_eq!(p_att_id, 903);
            assert_eq!(p_prov_id, Some("PROVIDER_COLLISION_1".to_string()));
            assert_eq!(p_file_name, Some("photo_a.jpg".to_string()));
            assert!(p_meta.contains("\"a\":1") || p_meta.contains("\"a\": 1"));
            assert!(p_meta.contains("\"b\":2") || p_meta.contains("\"b\": 2"));

            // 12. Verify unique index prevents future duplicate provider_attachment_id
            let dup_prov_insert = conn.execute(
                "INSERT INTO message_attachments (message_id, provider_attachment_id, created_at)
                 VALUES (701, 'PROVIDER_COLLISION_1', '2026-01-01')",
                [],
            );
            assert!(
                dup_prov_insert.is_err(),
                "Duplicate provider_attachment_id must violate unique index"
            );
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_v4_migration_overlapping_groups_conflicts_and_invalid_json() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(r#"
            CREATE TABLE channel_accounts (id INTEGER PRIMARY KEY);
            INSERT INTO channel_accounts (id) VALUES (1);

            CREATE TABLE channel_conversations (
                id INTEGER PRIMARY KEY,
                channel_account_id INTEGER,
                external_thread_id TEXT,
                kind TEXT
            );
            INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind) VALUES (10, 1, 'T1', 'direct');

            CREATE TABLE messages (
                id INTEGER PRIMARY KEY,
                conversation_id INTEGER,
                provider_message_id TEXT
            );
            INSERT INTO messages (id, conversation_id, provider_message_id) VALUES (100, 10, 'MID_100');

            CREATE TABLE message_attachments (
                id INTEGER PRIMARY KEY,
                message_id INTEGER,
                provider_attachment_id TEXT,
                attachment_type TEXT,
                file_name TEXT,
                mime_type TEXT,
                size_bytes INTEGER,
                remote_url TEXT,
                local_path TEXT,
                thumbnail_url TEXT,
                metadata_json TEXT,
                created_at TEXT
            );

            -- Connected component: Row 1001 shares remote_url with 1002, Row 1002 shares provider_id with 1003
            INSERT INTO message_attachments (id, message_id, provider_attachment_id, remote_url, metadata_json, created_at)
            VALUES (1001, 100, 'P1', 'https://cdn.example.com/shared.jpg', '{"key1": "val_a", "conflict_key": "survivor_val"}', '2026-01-01');

            INSERT INTO message_attachments (id, message_id, provider_attachment_id, remote_url, metadata_json, created_at)
            VALUES (1002, 100, 'P2', 'https://cdn.example.com/shared.jpg', '{"key2": "val_b", "conflict_key": "dup_val"}', '2026-01-01');

            INSERT INTO message_attachments (id, message_id, provider_attachment_id, remote_url, metadata_json, created_at)
            VALUES (1003, 100, 'P2', 'https://cdn.example.com/other.jpg', 'invalid json string payload', '2026-01-01');
        "#).unwrap();

        apply_migration_v4(&conn).unwrap();

        let v: u32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 4);

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM message_attachments WHERE message_id = 100",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "All overlapping connected rows must be merged into exactly 1 record"
        );

        let (survivor_id, prov_id, remote, meta): (i64, Option<String>, Option<String>, String) = conn.query_row(
            "SELECT id, provider_attachment_id, remote_url, metadata_json FROM message_attachments WHERE message_id = 100",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        ).unwrap();

        assert_eq!(survivor_id, 1001);
        assert_eq!(prov_id, Some("P1".to_string()));
        assert_eq!(
            remote,
            Some("https://cdn.example.com/shared.jpg".to_string())
        );
        assert!(meta.contains("\"key1\":\"val_a\"") || meta.contains("\"key1\": \"val_a\""));
        assert!(meta.contains("\"key2\":\"val_b\"") || meta.contains("\"key2\": \"val_b\""));
        assert!(
            meta.contains("\"conflict_key\":\"survivor_val\"")
                || meta.contains("\"conflict_key\": \"survivor_val\"")
        );
        assert!(
            meta.contains("\"conflict_key_dup_1002\":\"dup_val\"")
                || meta.contains("\"conflict_key_dup_1002\": \"dup_val\"")
        );
        assert!(meta.contains("invalid json string payload"));
        assert!(
            meta.contains("provider_attachment_id_aliases")
                && meta.contains("P1")
                && meta.contains("P2"),
            "All provider attachment ID aliases must be preserved losslessly in metadata_json"
        );
        assert!(
            meta.contains("remote_url_aliases")
                && meta.contains("https://cdn.example.com/shared.jpg")
                && meta.contains("https://cdn.example.com/other.jpg"),
            "All remote URL aliases must be preserved losslessly in metadata_json"
        );
        assert!(
            meta.contains("_merged_attachment_rows"),
            "All original rows must be recorded in _merged_attachment_rows"
        );
    }

    #[test]
    fn test_old_v3_database_repaired_to_v4() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            PRAGMA user_version = 3;
            CREATE TABLE channel_accounts (id INTEGER PRIMARY KEY);
            INSERT INTO channel_accounts (id) VALUES (1);

            CREATE TABLE message_attachments (
                id INTEGER PRIMARY KEY,
                message_id INTEGER,
                provider_attachment_id TEXT,
                attachment_type TEXT,
                file_name TEXT,
                mime_type TEXT,
                size_bytes INTEGER,
                remote_url TEXT,
                local_path TEXT,
                thumbnail_url TEXT,
                metadata_json TEXT,
                created_at TEXT
            );

            INSERT INTO message_attachments (id, message_id, remote_url, metadata_json, created_at)
            VALUES (2001, 50, 'https://cdn.example.com/dup.jpg', '{"x": 10}', '2026-01-01');

            INSERT INTO message_attachments (id, message_id, remote_url, metadata_json, created_at)
            VALUES (2002, 50, 'https://cdn.example.com/dup.jpg', '{"y": 20}', '2026-01-01');
        "#,
        )
        .unwrap();

        apply_migration_v4(&conn).unwrap();

        let v: u32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 4);

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM message_attachments WHERE message_id = 50",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        let (survivor_id, meta): (i64, String) = conn
            .query_row(
                "SELECT id, metadata_json FROM message_attachments WHERE message_id = 50",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(survivor_id, 2001);
        assert!(meta.contains("\"x\":10") || meta.contains("\"x\": 10"));
        assert!(meta.contains("\"y\":20") || meta.contains("\"y\": 20"));
    }

    #[test]
    fn test_existing_v2_to_v3_upgrade_preserves_data() {
        let conn = Connection::open_in_memory().unwrap();

        // Base V1 schema fixture
        conn.execute_batch(
            r#"
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
        "#,
        )
        .unwrap();

        // 1. Setup V2 database
        apply_migration_v2(&conn).unwrap();
        let v2: u32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
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
        let v3: u32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
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
        let v3_repeat: u32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v3_repeat, 3);
    }

    #[test]
    fn test_migration_failure_rollback_and_recovery() {
        let conn = Connection::open_in_memory().unwrap();

        // Setup base schema
        conn.execute_batch(r#"
            CREATE TABLE channel_identities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel TEXT NOT NULL,
                identity_namespace TEXT NOT NULL,
                external_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            INSERT INTO channel_identities (id, channel, identity_namespace, external_id, created_at, updated_at)
            VALUES (1, 'zalo', 'uid', 'U123', '2026-01-01', '2026-01-01');
            PRAGMA user_version = 1;
        "#).unwrap();

        // Simulate a savepoint transaction where an injected query fails
        let res = (|| -> rusqlite::Result<()> {
            conn.execute_batch("SAVEPOINT test_fail;")?;
            conn.execute_batch("INSERT INTO channel_identities (id, channel, identity_namespace, external_id, created_at, updated_at) VALUES (2, 'zalo', 'uid', 'U456', '2026-01-01', '2026-01-01');")?;
            // Injected error: invalid SQL
            conn.execute_batch("INSERT INTO non_existent_table VALUES (1);")?;
            conn.execute_batch("RELEASE SAVEPOINT test_fail;")?;
            Ok(())
        })();

        assert!(res.is_err());
        let _ = conn.execute_batch("ROLLBACK TO SAVEPOINT test_fail; RELEASE SAVEPOINT test_fail;");

        // Verify that row 2 was rolled back cleanly and row 1 remains
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM channel_identities", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        let v: u32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 1);
    }

    #[test]
    fn test_init_db_foreign_key_violation_rolls_back_cleanly() {
        let path = std::env::temp_dir().join(format!(
            "rustsale-fk-rollback-test-{}.db",
            rand::random::<u64>()
        ));
        {
            // 1. Create a database with an invalid orphaned foreign key record
            let conn = open_db_connection(path.to_str().unwrap()).unwrap();
            conn.execute_batch(r#"
                PRAGMA foreign_keys = OFF;
                CREATE TABLE leads (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL,
                    company TEXT NOT NULL,
                    status TEXT NOT NULL,
                    value REAL NOT NULL DEFAULT 0.0,
                    last_activity TEXT NOT NULL,
                    owner TEXT NOT NULL,
                    email TEXT NOT NULL,
                    phone TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE TABLE channel_accounts (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    channel TEXT NOT NULL,
                    provider_account_key TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE TABLE channel_conversations (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    channel_account_id INTEGER NOT NULL,
                    external_thread_id TEXT NOT NULL,
                    kind TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
                );
                CREATE TABLE messages (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    conversation_id INTEGER NOT NULL,
                    direction TEXT NOT NULL,
                    content_type TEXT NOT NULL,
                    ingested_at TEXT NOT NULL,
                    FOREIGN KEY(conversation_id) REFERENCES channel_conversations(id) ON DELETE CASCADE
                );
                -- Orphan message row with conversation_id = 99999
                INSERT INTO messages (id, conversation_id, direction, content_type, ingested_at)
                VALUES (1, 99999, 'inbound', 'text', '2026-01-01T00:00:00Z');
                PRAGMA user_version = 1;
            "#).unwrap();
        }

        // 3. init_db MUST fail and return SqliteFailure with Foreign Key violation details
        let init_res = init_db(path.to_str().unwrap());
        assert!(
            init_res.is_err(),
            "init_db must fail when foreign key violations exist"
        );
        let err_str = format!("{:?}", init_res.err().unwrap());
        assert!(
            err_str.contains("Foreign key check failed") || err_str.contains("foreign key"),
            "Error must contain foreign key diagnostic details: {}",
            err_str
        );

        // 4. Verify that the database was NOT committed to upgraded schema version
        {
            let conn = open_db_connection(path.to_str().unwrap()).unwrap();
            let v: u32 = conn
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .unwrap();
            // user_version must remain 1 because the migration transaction rolled back
            assert_eq!(v, 1, "Failed migration must roll back user_version");
        }
        let _ = std::fs::remove_file(&path);
    }
}
