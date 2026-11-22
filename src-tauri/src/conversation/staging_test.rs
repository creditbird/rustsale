use super::migration::migrate;
use super::repository::{ConversationRepository, StagedAttachmentRecord};
use super::staging::*;
use super::types::StageConversationClipboardImageRequest;
use crate::db::open_db_connection;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::os::windows::fs::symlink_file;
use std::sync::Arc;

struct AutoCleanDir(std::path::PathBuf);
impl AutoCleanDir {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("staging-test-{}", rand::random::<u64>()));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for AutoCleanDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn setup_test_stager() -> (ConversationAttachmentStager, AutoCleanDir) {
    let dir = AutoCleanDir::new();
    let db_path = dir.path().join("test.db");
    let conn = open_db_connection(db_path.to_str().unwrap()).unwrap();
    migrate(&conn).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at) VALUES (1, 'zalo', 'imei-a', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at) VALUES (1, 1, 'peer-a', 'direct', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    ).unwrap();
    drop(conn);

    let repo = Arc::new(ConversationRepository::new(
        db_path.to_str().unwrap().to_string(),
    ));
    let spool_root = dir.path().join("spool");
    let stager = ConversationAttachmentStager::new(spool_root, repo);
    (stager, dir)
}

#[tokio::test]
async fn stage_path_copies_bytes_into_spool_and_returns_opaque_id() {
    let (stager, dir) = setup_test_stager();
    let source_dir = dir.path().join("source");
    fs::create_dir_all(&source_dir).unwrap();
    let source_file = source_dir.join("hello.txt");
    fs::write(&source_file, b"hello world").unwrap();

    let res = stager
        .stage_paths(1, vec![source_file.to_str().unwrap().to_string()])
        .await
        .unwrap();

    assert_eq!(res.len(), 1);
    let item = &res[0];
    assert!(!item.staged_attachment_id.is_empty());
    assert_eq!(item.file_size_bytes, 11);
    assert_eq!(item.file_name, "hello.txt");

    // Source remains present
    assert!(source_file.exists());

    // Spool file exists under spool root
    let spool_file = stager
        .root()
        .join(format!("{}.dat", item.staged_attachment_id));
    assert!(spool_file.exists());
    assert_ne!(spool_file, source_file);
    assert_eq!(fs::read(&spool_file).unwrap(), b"hello world");
}

#[tokio::test]
async fn stage_path_rejects_file_larger_than_50_mib() {
    let (stager, dir) = setup_test_stager();
    let source_file = dir.path().join("large.bin");
    let f = fs::File::create(&source_file).unwrap();
    f.set_len(MAX_ATTACHMENT_BYTES + 1).unwrap();

    let err = stager
        .stage_paths(1, vec![source_file.to_str().unwrap().to_string()])
        .await
        .unwrap_err();

    assert_eq!(err.code, super::error::ConversationErrorCode::FileTooLarge);

    // No spool files remain
    let entries = fs::read_dir(stager.root()).unwrap().count();
    assert_eq!(entries, 0);
}

#[tokio::test]
async fn stage_clipboard_rejects_encoded_payload_over_10_mib() {
    let (stager, _dir) = setup_test_stager();
    let raw_bytes = vec![0u8; (MAX_CLIPBOARD_IMAGE_BYTES + 1) as usize];
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &raw_bytes);

    let req = StageConversationClipboardImageRequest {
        conversation_id: 1,
        file_name: "pasted.png".to_string(),
        mime_type: "image/png".to_string(),
        base64_data: encoded,
    };

    let err = stager.stage_clipboard_image(req).await.unwrap_err();
    assert_eq!(err.code, super::error::ConversationErrorCode::FileTooLarge);
}

#[tokio::test]
async fn discard_is_idempotent_when_file_is_already_missing() {
    let (stager, dir) = setup_test_stager();
    let source_file = dir.path().join("test.txt");
    fs::write(&source_file, b"data").unwrap();

    let res = stager
        .stage_paths(1, vec![source_file.to_str().unwrap().to_string()])
        .await
        .unwrap();

    let staged_id = &res[0].staged_attachment_id;
    let spool_file = stager.root().join(format!("{}.dat", staged_id));

    // Manually remove spool file
    fs::remove_file(&spool_file).unwrap();

    // Call discard twice
    stager.discard(staged_id).await.unwrap();
    stager.discard(staged_id).await.unwrap();
}

#[tokio::test]
async fn staging_rejects_symlink_source() {
    let (stager, dir) = setup_test_stager();
    let target_file = dir.path().join("real.txt");
    fs::write(&target_file, b"real data").unwrap();

    let symlink_path = dir.path().join("link.txt");
    #[cfg(unix)]
    let symlink_created = symlink(&target_file, &symlink_path).is_ok();
    #[cfg(windows)]
    let symlink_created = symlink_file(&target_file, &symlink_path).is_ok();

    if symlink_created {
        let err = stager
            .stage_paths(1, vec![symlink_path.to_str().unwrap().to_string()])
            .await
            .unwrap_err();

        assert_eq!(
            err.code,
            super::error::ConversationErrorCode::UnsupportedMedia
        );
    }
}

#[tokio::test]
async fn staging_enforces_one_gib_spool_quota() {
    let (stager, dir) = setup_test_stager();
    let db_path = dir.path().join("test.db");
    let repo = ConversationRepository::new(db_path.to_str().unwrap().to_string());

    // Insert active staged record totaling MAX_SPOOL_BYTES
    let rec = StagedAttachmentRecord {
        id: "rec-quota".to_string(),
        conversation_id: 1,
        staged_attachment_id: "stage-quota".to_string(),
        media_type: "file".to_string(),
        mime_type: "application/octet-stream".to_string(),
        file_name: "quota.bin".to_string(),
        file_size_bytes: MAX_SPOOL_BYTES,
        local_path: "/tmp/quota.bin".to_string(),
        thumbnail_path: None,
        width: None,
        height: None,
        duration_seconds: None,
        status: "staged".to_string(),
        expires_at: "2099-01-01T00:00:00.000Z".to_string(),
        created_at: "2026-08-01T00:00:00.000Z".to_string(),
        claimed_at: None,
    };
    repo.insert_staged_attachment(&rec).unwrap();

    // Next staging request fails with FileTooLarge
    let source_file = dir.path().join("new.txt");
    fs::write(&source_file, b"overflow").unwrap();

    let err = stager
        .stage_paths(1, vec![source_file.to_str().unwrap().to_string()])
        .await
        .unwrap_err();

    assert_eq!(err.code, super::error::ConversationErrorCode::FileTooLarge);
}

#[tokio::test]
async fn staging_rejects_batch_over_100_mib_without_partial_files() {
    let (stager, dir) = setup_test_stager();
    let first = dir.path().join("first.bin");
    let second = dir.path().join("second.bin");
    let third = dir.path().join("third.bin");

    fs::File::create(&first)
        .unwrap()
        .set_len(MAX_ATTACHMENT_BYTES)
        .unwrap();
    fs::File::create(&second)
        .unwrap()
        .set_len(MAX_ATTACHMENT_BYTES)
        .unwrap();
    fs::File::create(&third).unwrap().set_len(1).unwrap();

    let err = stager
        .stage_paths(
            1,
            vec![
                first.to_string_lossy().to_string(),
                second.to_string_lossy().to_string(),
                third.to_string_lossy().to_string(),
            ],
        )
        .await
        .unwrap_err();

    assert_eq!(err.code, super::error::ConversationErrorCode::FileTooLarge);
    assert!(err.message.contains("100 MiB"));
    assert_eq!(fs::read_dir(stager.root()).unwrap().count(), 0);
}

#[tokio::test]
async fn staging_errors_do_not_expose_source_paths() {
    let (stager, dir) = setup_test_stager();
    let source = dir.path().join("customer-secret-contract.pdf");

    let err = stager
        .stage_paths(1, vec![source.to_string_lossy().to_string()])
        .await
        .unwrap_err();

    assert_eq!(
        err.code,
        super::error::ConversationErrorCode::InvalidRequest
    );
    assert!(!err.message.contains("customer-secret-contract.pdf"));
    assert!(!err
        .message
        .contains(&dir.path().to_string_lossy().to_string()));
}

#[tokio::test]
async fn staging_detects_office_and_archive_document_mimes() {
    let (stager, dir) = setup_test_stager();
    let source_dir = dir.path().join("source_docs");
    fs::create_dir_all(&source_dir).unwrap();

    let docx_path = source_dir.join("test_document.docx");
    fs::write(&docx_path, b"PK\x03\x04mock_docx_bytes").unwrap();

    let xlsx_path = source_dir.join("financial_report.xlsx");
    fs::write(&xlsx_path, b"PK\x03\x04mock_xlsx_bytes").unwrap();

    let pptx_path = source_dir.join("presentation.pptx");
    fs::write(&pptx_path, b"PK\x03\x04mock_pptx_bytes").unwrap();

    let csv_path = source_dir.join("contacts.csv");
    fs::write(&csv_path, b"name,phone\nAlice,0901234567\n").unwrap();

    let res = stager
        .stage_paths(
            1,
            vec![
                docx_path.to_string_lossy().to_string(),
                xlsx_path.to_string_lossy().to_string(),
                pptx_path.to_string_lossy().to_string(),
                csv_path.to_string_lossy().to_string(),
            ],
        )
        .await
        .unwrap();

    assert_eq!(res.len(), 4);
    assert_eq!(res[0].media_type, "file");
    assert_eq!(
        res[0].mime_type,
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
    );
    assert_eq!(res[1].media_type, "file");
    assert_eq!(
        res[1].mime_type,
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    );
    assert_eq!(res[2].media_type, "file");
    assert_eq!(
        res[2].mime_type,
        "application/vnd.openxmlformats-officedocument.presentationml.presentation"
    );
    assert_eq!(res[3].media_type, "file");
    assert_eq!(res[3].mime_type, "text/csv");
}

#[tokio::test]
async fn staging_blocks_executable_file_extensions() {
    let (stager, dir) = setup_test_stager();
    let source_dir = dir.path().join("source_danger");
    fs::create_dir_all(&source_dir).unwrap();

    let exe_path = source_dir.join("malicious.exe");
    fs::write(&exe_path, b"MZ\x90\x00executable").unwrap();

    let err = stager
        .stage_paths(1, vec![exe_path.to_string_lossy().to_string()])
        .await
        .unwrap_err();

    assert_eq!(
        err.code,
        super::error::ConversationErrorCode::UnsupportedMedia
    );
    assert!(err.message.contains("Executable files are not supported"));
}
