use super::login::*;
use crate::conversation::whatsapp::migration::apply_whatsapp_migrations;
use rusqlite::Connection;
use tempfile::tempdir;

fn setup_test_db() -> Connection {
    let conn = Connection::open_in_memory().expect("open in-memory db");
    conn.execute_batch(include_str!("../schema.sql"))
        .expect("apply base conversation schema");
    apply_whatsapp_migrations(&conn).expect("apply whatsapp migrations");
    conn
}

#[tokio::test]
async fn test_start_qr_login_public_api() {
    let dir = tempdir().unwrap();
    let manager = WhatsAppLoginManager::new(dir.path().to_path_buf(), None);
    let attempt_id = "test-prod-attempt-1";

    manager.start_qr_login(attempt_id).await.unwrap();
    assert!(manager.is_active(attempt_id));

    let temp_db_path = manager.get_temp_db_path(attempt_id);
    assert!(temp_db_path.exists());

    manager.cancel_qr_login(attempt_id).unwrap();
    assert!(!manager.is_active(attempt_id));
    assert!(!temp_db_path.exists());
}

#[tokio::test]
async fn test_login_attempt_cancellation() {
    let dir = tempdir().unwrap();
    let manager = WhatsAppLoginManager::new_mock_with_dir(dir.path().to_path_buf());
    let attempt_id = "test-attempt-123";

    manager.start_mock_login(attempt_id).await.unwrap();
    assert!(manager.is_active(attempt_id));

    // Ensure temporary session DB file exists
    let temp_db_path = manager.get_temp_db_path(attempt_id);
    assert!(
        temp_db_path.exists(),
        "temp db file should exist while active"
    );

    manager.cancel_qr_login(attempt_id).unwrap();
    assert!(
        !manager.is_active(attempt_id),
        "manager should not report active after cancel"
    );
    assert!(
        !temp_db_path.exists(),
        "temp db file should be cleaned up immediately on cancel"
    );
}

#[tokio::test]
async fn test_login_attempt_qr_emission() {
    let dir = tempdir().unwrap();
    let manager = WhatsAppLoginManager::new_mock_with_dir(dir.path().to_path_buf());
    let attempt_id = "test-attempt-qr";

    let mut rx = manager.subscribe_events();
    manager.start_mock_login(attempt_id).await.unwrap();

    // Simulate emitting a QR event
    manager
        .emit_mock_qr(attempt_id, "2@mockqrpayload,12345,67890", 30, 1)
        .await
        .unwrap();

    let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .expect("timeout waiting for event")
        .expect("event received");

    match event {
        WhatsAppLoginEvent::QrCode(qr) => {
            assert_eq!(qr.attempt_id, attempt_id);
            assert_eq!(qr.qr_raw, "2@mockqrpayload,12345,67890");
            assert_eq!(qr.expires_in_sec, 30);
            assert_eq!(qr.sequence, 1);
        }
        other => panic!("expected QrCode event, got {:?}", other),
    }

    manager.cancel_qr_login(attempt_id).unwrap();
}

#[tokio::test]
async fn test_login_attempt_finalize_success() {
    let dir = tempdir().unwrap();
    let conn = setup_test_db();
    let manager = WhatsAppLoginManager::new_mock_with_dir(dir.path().to_path_buf());
    let attempt_id = "test-attempt-success";

    let mut rx = manager.subscribe_events();
    manager.start_mock_login(attempt_id).await.unwrap();

    let temp_db_path = manager.get_temp_db_path(attempt_id);
    assert!(temp_db_path.exists());

    // Complete login successfully
    let account_key = "84901234567";
    let phone_number = "84901234567";
    let push_name = "RustSale Store";
    let lid = Some("12345678901234@lid");

    let account_id = manager
        .finalize_login_success(
            &conn,
            attempt_id,
            account_key,
            phone_number,
            Some(push_name),
            lid,
            Some("https://avatar.url/wa.png"),
        )
        .unwrap();

    assert!(!manager.is_active(attempt_id), "attempt should be finished");
    assert!(
        !temp_db_path.exists(),
        "temp db file should be moved/renamed"
    );

    let final_db_path = manager.get_session_db_path(account_key);
    assert!(final_db_path.exists(), "final session db file should exist");

    // Check channel_accounts record
    let (ch_key, ch_name, status, avatar): (String, Option<String>, String, Option<String>) = conn
        .query_row(
            "SELECT provider_account_key, display_name, connection_status, avatar_url FROM channel_accounts WHERE id = ?1",
            [account_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(ch_key, account_key);
    assert_eq!(ch_name, Some(push_name.to_string()));
    assert_eq!(status, "connected");
    assert_eq!(avatar, Some("https://avatar.url/wa.png".to_string()));

    // Check whatsapp_sessions record
    let (wa_phone, wa_name, wa_db, is_valid, wa_lid): (String, Option<String>, String, i64, Option<String>) = conn
        .query_row(
            "SELECT phone_number, push_name, session_db_filename, is_valid, lid FROM whatsapp_sessions WHERE channel_account_id = ?1",
            [account_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .unwrap();
    assert_eq!(wa_phone, phone_number);
    assert_eq!(wa_name, Some(push_name.to_string()));
    assert_eq!(wa_db, format!("session_{}.db", account_key));
    assert_eq!(is_valid, 1);
    assert_eq!(wa_lid, lid.map(str::to_string));

    // Check event emission
    let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .expect("timeout waiting for event")
        .expect("event received");
    match event {
        WhatsAppLoginEvent::Success(succ) => {
            assert_eq!(succ.attempt_id, attempt_id);
            assert_eq!(succ.account_id, account_id);
            assert_eq!(succ.phone_number, phone_number);
            assert_eq!(succ.push_name.as_deref(), Some(push_name));
            assert_eq!(succ.lid.as_deref(), lid);
        }
        other => panic!("expected Success event, got {:?}", other),
    }
}

#[tokio::test]
async fn test_login_attempt_error_cleanup() {
    let dir = tempdir().unwrap();
    let manager = WhatsAppLoginManager::new_mock_with_dir(dir.path().to_path_buf());
    let attempt_id = "test-attempt-error";

    let mut rx = manager.subscribe_events();
    manager.start_mock_login(attempt_id).await.unwrap();

    let temp_db_path = manager.get_temp_db_path(attempt_id);
    assert!(temp_db_path.exists());

    manager.handle_login_error(attempt_id, "QR pairing timed out");

    assert!(!manager.is_active(attempt_id));
    assert!(!temp_db_path.exists(), "temp db should be deleted on error");

    let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
        .await
        .expect("timeout waiting for event")
        .expect("event received");
    match event {
        WhatsAppLoginEvent::Error(err) => {
            assert_eq!(err.attempt_id, attempt_id);
            assert_eq!(err.error, "QR pairing timed out");
        }
        other => panic!("expected Error event, got {:?}", other),
    }
}

#[tokio::test]
async fn test_multiple_concurrent_login_attempts_isolation() {
    let dir = tempdir().unwrap();
    let manager = WhatsAppLoginManager::new_mock_with_dir(dir.path().to_path_buf());

    let attempt_1 = "attempt-uuid-1";
    let attempt_2 = "attempt-uuid-2";

    manager.start_mock_login(attempt_1).await.unwrap();
    manager.start_mock_login(attempt_2).await.unwrap();

    assert!(manager.is_active(attempt_1));
    assert!(manager.is_active(attempt_2));

    let active_list = manager.list_active_attempts();
    assert_eq!(active_list.len(), 2);

    let temp_1 = manager.get_temp_db_path(attempt_1);
    let temp_2 = manager.get_temp_db_path(attempt_2);

    assert!(temp_1.exists());
    assert!(temp_2.exists());

    // Cancel attempt 1
    manager.cancel_qr_login(attempt_1).unwrap();
    assert!(!manager.is_active(attempt_1));
    assert!(manager.is_active(attempt_2));
    assert!(!temp_1.exists());
    assert!(temp_2.exists());

    // Cancel all
    manager.cancel_all();
    assert!(!manager.is_active(attempt_2));
    assert!(!temp_2.exists());
}

#[tokio::test]
async fn test_finalize_login_upsert_existing_account() {
    let dir = tempdir().unwrap();
    let conn = setup_test_db();
    let manager = WhatsAppLoginManager::new_mock_with_dir(dir.path().to_path_buf());

    let account_key = "84909999999";
    let phone_number = "84909999999";

    // First login
    manager.start_mock_login("attempt-1").await.unwrap();
    let account_id_1 = manager
        .finalize_login_success(
            &conn,
            "attempt-1",
            account_key,
            phone_number,
            Some("Original Name"),
            None,
            None,
        )
        .unwrap();

    // Re-login / update
    manager.start_mock_login("attempt-2").await.unwrap();
    let account_id_2 = manager
        .finalize_login_success(
            &conn,
            "attempt-2",
            account_key,
            phone_number,
            Some("Updated Name"),
            Some("99999@lid"),
            Some("https://newavatar.png"),
        )
        .unwrap();

    assert_eq!(
        account_id_1, account_id_2,
        "account id should remain the same on upsert"
    );

    let (display_name, lid, avatar): (Option<String>, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT s.push_name, s.lid, s.avatar_url FROM whatsapp_sessions s WHERE s.channel_account_id = ?1",
            [account_id_1],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();

    assert_eq!(display_name, Some("Updated Name".to_string()));
    assert_eq!(lid, Some("99999@lid".to_string()));
    assert_eq!(avatar, Some("https://newavatar.png".to_string()));
}
