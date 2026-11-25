use calamine::Data;
use rusqlite::Connection;
use std::io::Write;
use tempfile::NamedTempFile;

use super::importer::{
    execute_lead_import, normalize_email, normalize_vn_mobile, parse_lead_monetary_value,
};
use super::inspector::{cell_to_string, inspect_lead_file};
use super::types::{ColumnMapping, ConflictStrategy, LeadImportOptions};

#[test]
fn test_cell_to_string_variations() {
    assert_eq!(cell_to_string(&Data::Empty), "");
    assert_eq!(
        cell_to_string(&Data::String("  Nguyen Van A  ".into())),
        "Nguyen Van A"
    );
    assert_eq!(cell_to_string(&Data::Int(42)), "42");
    assert_eq!(cell_to_string(&Data::Float(50000000.0)), "50000000");
    assert_eq!(cell_to_string(&Data::Float(12.34)), "12.34");
    assert_eq!(cell_to_string(&Data::Bool(true)), "true");
}

#[test]
fn test_inspect_csv_file_success() {
    let mut file = NamedTempFile::new().unwrap();
    let csv_content = "\u{FEFF}Họ và tên,Số điện thoại,Email,Công ty\n\"Nguyễn Văn An\",\"0912345678\",\"an@example.com\",\"Công ty ABC\"\n\"Trần Thị Bình\",\"0987654321\",\"binh@example.com\",\"FPT\"\n";
    file.write_all(csv_content.as_bytes()).unwrap();

    let path = file.path().with_extension("csv");
    std::fs::copy(file.path(), &path).unwrap();

    let inspection = inspect_lead_file(&path, None).expect("Inspection should succeed");
    assert_eq!(inspection.file_type, "csv");
    assert_eq!(
        inspection.headers,
        vec!["Họ và tên", "Số điện thoại", "Email", "Công ty"]
    );
    assert_eq!(inspection.sample_rows.len(), 2);
    assert_eq!(inspection.sample_rows[0][0], "Nguyễn Văn An");
    assert_eq!(inspection.sample_rows[0][1], "0912345678");
    assert_eq!(inspection.total_rows_estimate, 2);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_inspect_nonexistent_file_returns_error() {
    let non_existent = std::path::PathBuf::from("/tmp/non_existent_file_xyz_12345.csv");
    let result = inspect_lead_file(&non_existent, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Tệp không tồn tại"));
}

#[test]
fn test_normalize_vn_mobile_matrix() {
    // Standard 10-digit formats
    assert_eq!(
        normalize_vn_mobile("0912345678"),
        Some("0912345678".to_string())
    );
    assert_eq!(
        normalize_vn_mobile("0868123456"),
        Some("0868123456".to_string())
    );
    assert_eq!(
        normalize_vn_mobile("0399123456"),
        Some("0399123456".to_string())
    );
    assert_eq!(
        normalize_vn_mobile("0777123456"),
        Some("0777123456".to_string())
    );
    assert_eq!(
        normalize_vn_mobile("0599123456"),
        Some("0599123456".to_string())
    );

    // International prefixes
    assert_eq!(
        normalize_vn_mobile("+84912345678"),
        Some("0912345678".to_string())
    );
    assert_eq!(
        normalize_vn_mobile("84912345678"),
        Some("0912345678".to_string())
    );
    assert_eq!(
        normalize_vn_mobile("0084912345678"),
        Some("0912345678".to_string())
    );

    // Disambiguate domestic 084 truncated by Excel from country codes (R3)
    assert_eq!(
        normalize_vn_mobile("841234567"),
        Some("0841234567".to_string())
    );
    assert_eq!(
        normalize_vn_mobile("+84841234567"),
        Some("0841234567".to_string())
    );
    assert_eq!(
        normalize_vn_mobile("84841234567"),
        Some("0841234567".to_string())
    );
    assert_eq!(
        normalize_vn_mobile("0084841234567"),
        Some("0841234567".to_string())
    );

    // Missing leading zero (Excel numerical truncation)
    assert_eq!(
        normalize_vn_mobile("912345678"),
        Some("0912345678".to_string())
    );
    assert_eq!(
        normalize_vn_mobile("868123456"),
        Some("0868123456".to_string())
    );
    assert_eq!(
        normalize_vn_mobile("399123456"),
        Some("0399123456".to_string())
    );

    // Separators and spaces
    assert_eq!(
        normalize_vn_mobile("0912.345.678"),
        Some("0912345678".to_string())
    );
    assert_eq!(
        normalize_vn_mobile("0912 345 678"),
        Some("0912345678".to_string())
    );
    assert_eq!(
        normalize_vn_mobile("(+84) 912-345-678"),
        Some("0912345678".to_string())
    );

    // Invalid numbers
    assert_eq!(normalize_vn_mobile("0243123456"), None); // Landline
    assert_eq!(normalize_vn_mobile("12345"), None); // Too short
    assert_eq!(normalize_vn_mobile("0912345678999"), None); // Too long
    assert_eq!(normalize_vn_mobile("abcdef"), None); // Non-numeric
}

#[test]
fn test_normalize_email() {
    assert_eq!(
        normalize_email("  Test@Example.COM  "),
        Some("test@example.com".to_string())
    );
    assert_eq!(normalize_email("invalid_email"), None);
    assert_eq!(normalize_email("a@b"), None);
    assert_eq!(normalize_email(""), None);
}

fn create_test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute("PRAGMA foreign_keys = ON;", []).unwrap();
    conn.execute(
        "CREATE TABLE IF NOT EXISTS categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        )",
        [],
    )
    .unwrap();

    conn.execute(
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
            currency TEXT NOT NULL DEFAULT 'VND'
        )",
        [],
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE IF NOT EXISTS lead_categories (
            lead_id INTEGER NOT NULL,
            category_id INTEGER NOT NULL,
            PRIMARY KEY (lead_id, category_id),
            FOREIGN KEY (lead_id) REFERENCES leads(id) ON DELETE CASCADE,
            FOREIGN KEY (category_id) REFERENCES categories(id) ON DELETE CASCADE
        )",
        [],
    )
    .unwrap();

    conn.execute("CREATE INDEX idx_leads_phone ON leads(phone)", [])
        .unwrap();
    conn.execute(
        "CREATE INDEX idx_leads_email_normalized ON leads(lower(trim(email)))",
        [],
    )
    .unwrap();

    conn
}

#[test]
fn test_execute_lead_import_skip_and_error_quarantine() {
    let mut conn = create_test_db();

    // Create category 99 so it exists
    conn.execute("INSERT INTO categories (id, name) VALUES (99, 'VIP')", [])
        .unwrap();

    // Insert an existing lead to test deduplication
    conn.execute(
        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, currency)
         VALUES ('Old Lead', 'Old Co', 'Contacted', 1000.0, '2026-01-01', 'Self', 'old@lead.com', '0912345678', 'VND')",
        [],
    ).unwrap();

    let mut file = NamedTempFile::new().unwrap();
    let csv_content = "Tên,Điện thoại,Email,Công ty,Giá trị\n\
        \"Khách Mới\",\"0987654321\",\"new@lead.com\",\"Công ty Mới\",\"50000000\"\n\
        \"\",\"0977111222\",\"noname@lead.com\",\"Co\",\"0\"\n\
        \"Khách Trùng\",\"0912345678\",\"dup@lead.com\",\"Co\",\"0\"\n\
        \"Khách Rác Số\",\"0123\",\"\",\"Co\",\"0\"\n";
    file.write_all(csv_content.as_bytes()).unwrap();

    let path = file.path().with_extension("csv");
    std::fs::copy(file.path(), &path).unwrap();

    let options = LeadImportOptions {
        file_path: path.to_string_lossy().to_string(),
        sheet_name: None,
        has_header: true,
        header_row_index: 0,
        mapping: ColumnMapping {
            name_col: Some(0),
            phone_col: Some(1),
            email_col: Some(2),
            company_col: Some(3),
            value_col: Some(4),
            address_col: None,
            about_col: None,
            status_col: None,
            owner_col: None,
        },
        conflict_strategy: ConflictStrategy::Skip,
        default_category_id: Some(99),
        default_status: Some("New".to_string()),
        default_owner: Some("Self".to_string()),
        default_currency: Some("VND".to_string()),
        normalize_phone: true,
    };

    let mut progress_ticks = 0;
    let summary = execute_lead_import(&mut conn, &options, |_progress| {
        progress_ticks += 1;
    })
    .expect("Import should run");

    assert_eq!(summary.total_rows, 4);
    assert_eq!(summary.inserted, 1);
    assert_eq!(summary.skipped_duplicates, 1); // 0912345678 matched Old Lead
    assert_eq!(summary.failed, 2); // 1 missing name, 1 invalid phone without email
    assert_eq!(summary.error_rows.len(), 2);
    assert!(summary.error_rows[0]
        .reason
        .contains("Họ và tên không được để trống"));
    assert!(summary.error_rows[1]
        .reason
        .contains("không đúng định dạng di động VN"));
    assert!(progress_ticks >= 1);

    // Verify category assigned to newly inserted lead
    let cat_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM lead_categories WHERE category_id = 99",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(cat_count, 1);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_execute_lead_import_overwrite_conflict() {
    let mut conn = create_test_db();

    conn.execute(
        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, currency)
         VALUES ('Nguyen Old', 'Old Co', 'New', 1000.0, '2026-01-01', 'Self', 'old@test.com', '0912345678', 'VND')",
        [],
    ).unwrap();

    let mut file = NamedTempFile::new().unwrap();
    let csv_content = "Name,Phone,Email,Company,Value\n\
        \"Nguyen Updated\",\"0912345678\",\"old@test.com\",\"New Upgraded Co\",\"99000000\"\n";
    file.write_all(csv_content.as_bytes()).unwrap();

    let path = file.path().with_extension("csv");
    std::fs::copy(file.path(), &path).unwrap();

    let options = LeadImportOptions {
        file_path: path.to_string_lossy().to_string(),
        sheet_name: None,
        has_header: true,
        header_row_index: 0,
        mapping: ColumnMapping {
            name_col: Some(0),
            phone_col: Some(1),
            email_col: Some(2),
            company_col: Some(3),
            value_col: Some(4),
            address_col: None,
            about_col: None,
            status_col: None,
            owner_col: None,
        },
        conflict_strategy: ConflictStrategy::Overwrite,
        default_category_id: None,
        default_status: None,
        default_owner: None,
        default_currency: None,
        normalize_phone: true,
    };

    let summary = execute_lead_import(&mut conn, &options, |_| {}).unwrap();
    assert_eq!(summary.total_rows, 1);
    assert_eq!(summary.updated, 1);
    assert_eq!(summary.inserted, 0);

    let (name, company, value): (String, String, f64) = conn
        .query_row(
            "SELECT name, company, value FROM leads WHERE phone = '0912345678'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();

    assert_eq!(name, "Nguyen Updated");
    assert_eq!(company, "New Upgraded Co");
    assert_eq!(value, 99000000.0);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_generate_and_inspect_csv_template() {
    use super::template::generate_csv_lead_template;

    let csv_bytes = generate_csv_lead_template();
    assert!(csv_bytes.starts_with(b"\xEF\xBB\xBF")); // BOM

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(&csv_bytes).unwrap();
    let path = file.path().with_extension("csv");
    std::fs::copy(file.path(), &path).unwrap();

    let inspection = inspect_lead_file(&path, None).expect("Should inspect generated CSV template");
    assert_eq!(inspection.file_type, "csv");
    assert_eq!(inspection.headers.len(), 8);
    assert_eq!(inspection.headers[0], "Họ và tên (*)");
    assert_eq!(inspection.headers[1], "Số điện thoại");
    assert_eq!(inspection.sample_rows.len(), 2);
    assert_eq!(inspection.sample_rows[0][0], "Nguyễn Văn An");
    assert_eq!(inspection.sample_rows[0][1], "0912345678");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_generate_and_inspect_excel_template() {
    use super::template::generate_excel_lead_template;

    let excel_bytes = generate_excel_lead_template().expect("Should generate excel template");
    assert!(!excel_bytes.is_empty());

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(&excel_bytes).unwrap();
    let path = file.path().with_extension("xlsx");
    std::fs::copy(file.path(), &path).unwrap();

    let inspection =
        inspect_lead_file(&path, None).expect("Should inspect generated XLSX template");
    assert_eq!(inspection.file_type, "excel");
    assert_eq!(inspection.sheets, vec!["Leads".to_string()]);
    assert_eq!(inspection.headers.len(), 8);
    assert_eq!(inspection.headers[0], "Họ và tên (*)");
    assert_eq!(inspection.headers[1], "Số điện thoại");
    assert_eq!(inspection.sample_rows.len(), 2);
    assert_eq!(inspection.sample_rows[0][0], "Nguyễn Văn An");
    assert_eq!(inspection.sample_rows[0][1], "0912345678");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_execute_lead_import_overwrite_preserves_unmapped_fields_and_currency() {
    let mut conn = create_test_db();

    // Lead created previously in USD, owned by Alice with status Won
    conn.execute(
        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, currency)
         VALUES ('Alice Old', 'Alice Corp', 'Won', 1000.0, '2026-01-01', 'Alice', 'alice@corp.com', '0912345678', 'USD')",
        [],
    ).unwrap();

    let mut file = NamedTempFile::new().unwrap();
    // CSV only supplies Name and Phone, omitting status, owner, value, currency
    let csv_content = "Name,Phone\n\"Alice Renamed\",\"0912345678\"\n";
    file.write_all(csv_content.as_bytes()).unwrap();

    let path = file.path().with_extension("csv");
    std::fs::copy(file.path(), &path).unwrap();

    let options = LeadImportOptions {
        file_path: path.to_string_lossy().to_string(),
        sheet_name: None,
        has_header: true,
        header_row_index: 0,
        mapping: ColumnMapping {
            name_col: Some(0),
            phone_col: Some(1),
            email_col: None,
            company_col: None,
            value_col: None,
            address_col: None,
            about_col: None,
            status_col: None,
            owner_col: None,
        },
        conflict_strategy: ConflictStrategy::Overwrite,
        default_category_id: None,
        default_status: Some("New".to_string()),
        default_owner: Some("Self".to_string()),
        default_currency: Some("VND".to_string()),
        normalize_phone: true,
    };

    let summary = execute_lead_import(&mut conn, &options, |_| {}).unwrap();
    assert_eq!(summary.total_rows, 1);
    assert_eq!(summary.updated, 1);

    // Verify R2: status 'Won', owner 'Alice', currency 'USD', value 1000.0 are PRESERVED!
    let (name, status, owner, currency, value): (String, String, String, String, f64) = conn
        .query_row(
            "SELECT name, status, owner, currency, value FROM leads WHERE phone = '0912345678'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();

    assert_eq!(name, "Alice Renamed");
    assert_eq!(status, "Won");
    assert_eq!(owner, "Alice");
    assert_eq!(currency, "USD");
    assert_eq!(value, 1000.0);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_category_not_found_rolls_back_transaction() {
    let mut conn = create_test_db();

    let mut file = NamedTempFile::new().unwrap();
    let csv_content = "Name,Phone\n\"Bob\",\"0987654321\"\n";
    file.write_all(csv_content.as_bytes()).unwrap();

    let path = file.path().with_extension("csv");
    std::fs::copy(file.path(), &path).unwrap();

    let options = LeadImportOptions {
        file_path: path.to_string_lossy().to_string(),
        sheet_name: None,
        has_header: true,
        header_row_index: 0,
        mapping: ColumnMapping {
            name_col: Some(0),
            phone_col: Some(1),
            email_col: None,
            company_col: None,
            value_col: None,
            address_col: None,
            about_col: None,
            status_col: None,
            owner_col: None,
        },
        conflict_strategy: ConflictStrategy::Skip,
        default_category_id: Some(99999), // Category 99999 does not exist
        default_status: None,
        default_owner: None,
        default_currency: None,
        normalize_phone: true,
    };

    // R5: Category validation must fail and abort atomically
    let result = execute_lead_import(&mut conn, &options, |_| {});
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("không tồn tại"));

    // Verify 0 leads inserted (transaction rolled back)
    let lead_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM leads", [], |r| r.get(0))
        .unwrap();
    assert_eq!(lead_count, 0);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_quarantine_preserves_original_source_row_numbers() {
    let mut conn = create_test_db();

    let mut file = NamedTempFile::new().unwrap();
    // Row 1: Header
    // Row 2: Valid lead
    // Row 3: Blank row
    // Row 4: Invalid lead (empty name)
    // Row 5: Blank row
    // Row 6: Invalid lead (bad phone, no email)
    let csv_content = "Name,Phone\nValid Lead,0911222333\n\n,0922333444\n\nBad Phone Lead,12345\n";
    file.write_all(csv_content.as_bytes()).unwrap();

    let path = file.path().with_extension("csv");
    std::fs::copy(file.path(), &path).unwrap();

    let options = LeadImportOptions {
        file_path: path.to_string_lossy().to_string(),
        sheet_name: None,
        has_header: true,
        header_row_index: 0,
        mapping: ColumnMapping {
            name_col: Some(0),
            phone_col: Some(1),
            email_col: None,
            company_col: None,
            value_col: None,
            address_col: None,
            about_col: None,
            status_col: None,
            owner_col: None,
        },
        conflict_strategy: ConflictStrategy::Skip,
        default_category_id: None,
        default_status: None,
        default_owner: None,
        default_currency: None,
        normalize_phone: true,
    };

    let summary = execute_lead_import(&mut conn, &options, |_| {}).unwrap();
    assert_eq!(summary.inserted, 1);
    assert_eq!(summary.failed, 2);
    assert_eq!(summary.error_rows.len(), 2);

    // R6: Source row numbers must be exact (Row 4 and Row 6) despite blank rows
    assert_eq!(summary.error_rows[0].row_number, 4);
    assert_eq!(summary.error_rows[1].row_number, 6);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_progress_emitted_for_skipped_and_failed_rows() {
    let mut conn = create_test_db();

    // Generate CSV with 110 invalid rows (all missing name)
    let mut csv_content = String::from("Name,Phone\n");
    for _ in 0..110 {
        csv_content.push_str(",0912345678\n");
    }

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(csv_content.as_bytes()).unwrap();

    let path = file.path().with_extension("csv");
    std::fs::copy(file.path(), &path).unwrap();

    let options = LeadImportOptions {
        file_path: path.to_string_lossy().to_string(),
        sheet_name: None,
        has_header: true,
        header_row_index: 0,
        mapping: ColumnMapping {
            name_col: Some(0),
            phone_col: Some(1),
            email_col: None,
            company_col: None,
            value_col: None,
            address_col: None,
            about_col: None,
            status_col: None,
            owner_col: None,
        },
        conflict_strategy: ConflictStrategy::Skip,
        default_category_id: None,
        default_status: None,
        default_owner: None,
        default_currency: None,
        normalize_phone: false,
    };

    let mut progress_calls = Vec::new();
    let summary = execute_lead_import(&mut conn, &options, |p| {
        progress_calls.push(p);
    })
    .unwrap();

    assert_eq!(summary.failed, 110);
    // R4: Intermediate progress callbacks (at 50, 100, 110) + final 100% callback
    assert!(progress_calls.len() >= 3);
    assert_eq!(progress_calls.last().unwrap().percentage, 100);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_parse_lead_monetary_value_formats() {
    // Vietnamese dotted thousands format (VND)
    assert_eq!(
        parse_lead_monetary_value("1.000.000", "VND"),
        Ok(Some(1_000_000.0))
    );
    assert_eq!(
        parse_lead_monetary_value("50.000", "VND"),
        Ok(Some(50_000.0))
    );
    assert_eq!(
        parse_lead_monetary_value("1.000.000,50", "VND"),
        Ok(Some(1_000_000.50))
    );

    // International comma separated format (USD and VND)
    assert_eq!(
        parse_lead_monetary_value("1,000,000.50", "USD"),
        Ok(Some(1_000_000.50))
    );
    assert_eq!(
        parse_lead_monetary_value("1,000,000", "USD"),
        Ok(Some(1_000_000.0))
    );
    assert_eq!(
        parse_lead_monetary_value("1,000,000", "VND"),
        Ok(Some(1_000_000.0))
    );

    // Currency symbols matching expected currency
    assert_eq!(
        parse_lead_monetary_value("500.000 ₫", "VND"),
        Ok(Some(500_000.0))
    );
    assert_eq!(
        parse_lead_monetary_value("500000 đ", "VND"),
        Ok(Some(500_000.0))
    );
    assert_eq!(
        parse_lead_monetary_value("10.000.000 VND", "VND"),
        Ok(Some(10_000_000.0))
    );
    assert_eq!(
        parse_lead_monetary_value("1000 $", "USD"),
        Ok(Some(1_000.0))
    );
    assert_eq!(parse_lead_monetary_value("$1000", "USD"), Ok(Some(1_000.0)));
    assert_eq!(
        parse_lead_monetary_value("1000 USD", "USD"),
        Ok(Some(1_000.0))
    );
    assert_eq!(
        parse_lead_monetary_value("2 500 000", "VND"),
        Ok(Some(2_500_000.0))
    );

    // F1: Rejection of conflicting currency markers
    assert!(parse_lead_monetary_value("1000 $", "VND").is_err());
    assert!(parse_lead_monetary_value("$1000", "VND").is_err());
    assert!(parse_lead_monetary_value("1000 USD", "VND").is_err());
    assert!(parse_lead_monetary_value("500.000 ₫", "USD").is_err());
    assert!(parse_lead_monetary_value("500.000 đ", "USD").is_err());
    assert!(parse_lead_monetary_value("10.000.000 VND", "USD").is_err());

    // Empty or blank values
    assert_eq!(parse_lead_monetary_value("", "VND"), Ok(None));
    assert_eq!(parse_lead_monetary_value("   ", "VND"), Ok(None));
    assert_eq!(parse_lead_monetary_value("VND", "VND"), Ok(None));
    assert_eq!(parse_lead_monetary_value("USD", "USD"), Ok(None));

    // Explicit zero
    assert_eq!(parse_lead_monetary_value("0", "VND"), Ok(Some(0.0)));
    assert_eq!(parse_lead_monetary_value("0.0", "USD"), Ok(Some(0.0)));

    // Invalid / malformed inputs must return Err
    assert!(parse_lead_monetary_value("abc", "VND").is_err());
    assert!(parse_lead_monetary_value("12abc34", "VND").is_err());
    assert!(parse_lead_monetary_value("-1000", "VND").is_err());
    assert!(parse_lead_monetary_value("1..000", "VND").is_err());
    assert!(parse_lead_monetary_value("1,,000", "VND").is_err());
    assert!(parse_lead_monetary_value("1.00.00", "VND").is_err());
    assert!(parse_lead_monetary_value("1,00,00", "VND").is_err());
    assert!(parse_lead_monetary_value("1.000..00", "VND").is_err());
}

#[test]
fn test_execute_import_rejects_malformed_monetary_value_and_quarantines() {
    let mut conn = create_test_db();

    // Insert an existing lead with value = 5_000_000
    conn.execute(
        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, currency)
         VALUES ('Existing Lead', 'Old Corp', 'New', 5000000.0, '2026-09-09', 'Self', 'existing@example.com', '0911111111', 'VND')",
        [],
    ).unwrap();

    let mut file = NamedTempFile::new().unwrap();
    // Row 1: Valid import with VN dotted value
    // Row 2: Overwrite existing lead with malformed monetary value "INVALID_VAL"
    let csv_content =
        "Name,Phone,Value\nNew Lead,0922222222,1.000.000\nExisting Lead,0911111111,INVALID_VAL\n";
    file.write_all(csv_content.as_bytes()).unwrap();

    let path = file.path().with_extension("csv");
    std::fs::copy(file.path(), &path).unwrap();

    let options = LeadImportOptions {
        file_path: path.to_string_lossy().to_string(),
        sheet_name: None,
        has_header: true,
        header_row_index: 0,
        mapping: ColumnMapping {
            name_col: Some(0),
            phone_col: Some(1),
            email_col: None,
            company_col: None,
            value_col: Some(2),
            address_col: None,
            about_col: None,
            status_col: None,
            owner_col: None,
        },
        conflict_strategy: ConflictStrategy::Overwrite,
        default_category_id: None,
        default_status: None,
        default_owner: None,
        default_currency: Some("VND".to_string()),
        normalize_phone: true,
    };

    let summary = execute_lead_import(&mut conn, &options, |_| {}).unwrap();
    assert_eq!(summary.inserted, 1);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.error_rows.len(), 1);
    assert!(summary.error_rows[0]
        .reason
        .contains("không đúng định dạng số"));

    // Check newly inserted lead has parsed value 1_000_000.0
    let new_val: f64 = conn
        .query_row(
            "SELECT value FROM leads WHERE phone = '0922222222'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(new_val, 1_000_000.0);

    // Verify existing lead value was NOT erased or modified to 0
    let existing_val: f64 = conn
        .query_row(
            "SELECT value FROM leads WHERE phone = '0911111111'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(existing_val, 5_000_000.0);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_csv_multiline_notes_and_correct_line_numbers() {
    let mut conn = create_test_db();

    let mut file = NamedTempFile::new().unwrap();
    // Row 1: Header
    // Row 2-4: Lead 1 with quoted multiline note containing commas, quotes, and newlines
    // Row 5: Lead 2 normal
    // Row 6: Lead 3 invalid (bad phone 12345, no email) -> should report error on line 6!
    let csv_content = "Name,Phone,Notes\n\"Alice Smith\",\"0912345678\",\"First line note\nSecond line with, comma\nThird line with \"\"quotes\"\"\"\n\"Bob Tran\",\"0987654321\",\"Normal note\"\n\"Charlie\",\"12345\",\"\"\n";
    file.write_all(csv_content.as_bytes()).unwrap();

    let path = file.path().with_extension("csv");
    std::fs::copy(file.path(), &path).unwrap();

    let options = LeadImportOptions {
        file_path: path.to_string_lossy().to_string(),
        sheet_name: None,
        has_header: true,
        header_row_index: 0,
        mapping: ColumnMapping {
            name_col: Some(0),
            phone_col: Some(1),
            email_col: None,
            company_col: None,
            value_col: None,
            address_col: None,
            about_col: Some(2),
            status_col: None,
            owner_col: None,
        },
        conflict_strategy: ConflictStrategy::Skip,
        default_category_id: None,
        default_status: None,
        default_owner: None,
        default_currency: None,
        normalize_phone: true,
    };

    let summary = execute_lead_import(&mut conn, &options, |_| {}).unwrap();
    assert_eq!(summary.inserted, 2);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.error_rows.len(), 1);

    // Exact error row number for Charlie must be 6
    assert_eq!(summary.error_rows[0].row_number, 6);

    // Check Alice's about field preserved exact multiline content
    let alice_notes: String = conn
        .query_row(
            "SELECT about FROM leads WHERE phone = '0912345678'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(alice_notes
        .contains("First line note\nSecond line with, comma\nThird line with \"quotes\""));

    // Check Bob was imported correctly without being affected by Alice's multiline field
    let bob_name: String = conn
        .query_row(
            "SELECT name FROM leads WHERE phone = '0987654321'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(bob_name, "Bob Tran");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_database_write_failure_rolls_back_entire_transaction() {
    let mut conn = create_test_db();

    // Create a strict database trigger that intentionally fails when name == 'POISON_ROW'
    conn.execute(
        "CREATE TRIGGER trigger_poison_row
         BEFORE INSERT ON leads
         WHEN NEW.name = 'POISON_ROW'
         BEGIN
             SELECT RAISE(ABORT, 'Custom DB constraint violation');
         END;",
        [],
    )
    .unwrap();

    let mut file = NamedTempFile::new().unwrap();
    // Row 1: Valid Lead (would succeed if not rolled back)
    // Row 2: Poison Row (will cause insert_lead_stmt.execute to fail)
    let csv_content = "Name,Phone\nValid Lead,0912345678\nPOISON_ROW,0987654321\n";
    file.write_all(csv_content.as_bytes()).unwrap();

    let path = file.path().with_extension("csv");
    std::fs::copy(file.path(), &path).unwrap();

    let options = LeadImportOptions {
        file_path: path.to_string_lossy().to_string(),
        sheet_name: None,
        has_header: true,
        header_row_index: 0,
        mapping: ColumnMapping {
            name_col: Some(0),
            phone_col: Some(1),
            email_col: None,
            company_col: None,
            value_col: None,
            address_col: None,
            about_col: None,
            status_col: None,
            owner_col: None,
        },
        conflict_strategy: ConflictStrategy::Skip,
        default_category_id: None,
        default_status: None,
        default_owner: None,
        default_currency: None,
        normalize_phone: true,
    };

    // R2: DB error must propagate as Err, not return Ok(summary)
    let result = execute_lead_import(&mut conn, &options, |_| {});
    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("Lỗi chèn CSDL"));

    // R2: All previous mutations in the batch must be rolled back completely
    let lead_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM leads", [], |r| r.get(0))
        .unwrap();
    assert_eq!(lead_count, 0, "No leads should remain in DB after rollback");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_default_currency_validation_upfront() {
    let mut conn = create_test_db();

    let mut file = NamedTempFile::new().unwrap();
    let csv_content = "Name,Phone\nValid Lead,0912345678\n";
    file.write_all(csv_content.as_bytes()).unwrap();

    let path = file.path().with_extension("csv");
    std::fs::copy(file.path(), &path).unwrap();

    let options = LeadImportOptions {
        file_path: path.to_string_lossy().to_string(),
        sheet_name: None,
        has_header: true,
        header_row_index: 0,
        mapping: ColumnMapping {
            name_col: Some(0),
            phone_col: Some(1),
            email_col: None,
            company_col: None,
            value_col: None,
            address_col: None,
            about_col: None,
            status_col: None,
            owner_col: None,
        },
        conflict_strategy: ConflictStrategy::Skip,
        default_category_id: None,
        default_status: None,
        default_owner: None,
        default_currency: Some("EUR".to_string()),
        normalize_phone: true,
    };

    let result = execute_lead_import(&mut conn, &options, |_| {});
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("Đơn vị tiền tệ mặc định 'EUR' không hợp lệ: chỉ hỗ trợ VND hoặc USD."));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_overwrite_updates_currency_with_mapped_value_and_preserves_on_unmapped() {
    let mut conn = create_test_db();

    // Lead 1: Existing lead denominated in USD with value 100.0
    conn.execute(
        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, currency)
         VALUES ('USD Lead', 'Global Co', 'New', 100.0, '2026-09-09', 'Self', 'usd@example.com', '0911111111', 'USD')",
        [],
    ).unwrap();

    // Lead 2: Existing lead denominated in USD with value 250.0
    conn.execute(
        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, currency)
         VALUES ('Preserved Lead', 'Global Co', 'New', 250.0, '2026-09-09', 'Self', 'preserved@example.com', '0922222222', 'USD')",
        [],
    ).unwrap();

    // CSV with Overwrite:
    // Row 1: Overwrites Lead 1 with mapped value 1,000,000 (denominated in VND import)
    // Row 2: Overwrites Lead 2 with malformed separator "1..000" (should quarantine, leave 250 USD untouched)
    // Row 3: Overwrites Lead 2 with empty value (should preserve 250.0 USD)
    let mut file = NamedTempFile::new().unwrap();
    let csv_content =
        "Name,Phone,Value\nUSD Lead,0911111111,1.000.000\nPreserved Lead,0922222222,1..000\n";
    file.write_all(csv_content.as_bytes()).unwrap();

    let path = file.path().with_extension("csv");
    std::fs::copy(file.path(), &path).unwrap();

    let options = LeadImportOptions {
        file_path: path.to_string_lossy().to_string(),
        sheet_name: None,
        has_header: true,
        header_row_index: 0,
        mapping: ColumnMapping {
            name_col: Some(0),
            phone_col: Some(1),
            email_col: None,
            company_col: None,
            value_col: Some(2),
            address_col: None,
            about_col: None,
            status_col: None,
            owner_col: None,
        },
        conflict_strategy: ConflictStrategy::Overwrite,
        default_category_id: None,
        default_status: None,
        default_owner: None,
        default_currency: Some("VND".to_string()),
        normalize_phone: true,
    };

    let summary = execute_lead_import(&mut conn, &options, |_| {}).unwrap();
    assert_eq!(summary.updated, 1);
    assert_eq!(summary.failed, 1); // "1..000" quarantined!
    assert!(summary.error_rows[0]
        .reason
        .contains("chứa ký tự phân cách lặp"));

    // R1: Lead 1 value updated to 1_000_000.0 AND currency updated to VND (not retained as 1,000,000 USD!)
    let (lead1_val, lead1_curr): (f64, String) = conn
        .query_row(
            "SELECT value, currency FROM leads WHERE phone = '0911111111'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(lead1_val, 1_000_000.0);
    assert_eq!(lead1_curr, "VND");

    // R2: Lead 2 quarantined due to "1..000", value remains 250.0 and currency remains USD
    let (lead2_val, lead2_curr): (f64, String) = conn
        .query_row(
            "SELECT value, currency FROM leads WHERE phone = '0922222222'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(lead2_val, 250.0);
    assert_eq!(lead2_curr, "USD");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_canonical_phone_duplicate_matching_across_formats() {
    let mut conn = create_test_db();

    // Insert 4 existing leads with various real-world formatted numbers:
    // Lead 1: Dotted format "0912.345.678"
    // Lead 2: Spaced format "0912 345 679"
    // Lead 3: Parenthesized international format "(+84) 912-345-680"
    // Lead 4: Plus international format "+84912345681"
    conn.execute(
        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, currency)
         VALUES ('Old Lead 1', 'Corp 1', 'New', 100.0, '2026-09-09', 'Self', '', '0912.345.678', 'VND'),
                ('Old Lead 2', 'Corp 2', 'New', 100.0, '2026-09-09', 'Self', '', '0912 345 679', 'VND'),
                ('Old Lead 3', 'Corp 3', 'New', 100.0, '2026-09-09', 'Self', '', '(+84) 912-345-680', 'VND'),
                ('Old Lead 4', 'Corp 4', 'New', 100.0, '2026-09-09', 'Self', '', '+84912345681', 'VND')",
        [],
    ).unwrap();

    let initial_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM leads", [], |r| r.get(0))
        .unwrap();
    assert_eq!(initial_count, 4);

    let mut file = NamedTempFile::new().unwrap();
    // CSV rows with standard unformatted domestic format
    let csv_content = "Name,Phone\nNew Name 1,0912345678\nNew Name 2,0912345679\nNew Name 3,0912345680\nNew Name 4,0912345681\n";
    file.write_all(csv_content.as_bytes()).unwrap();

    let path = file.path().with_extension("csv");
    std::fs::copy(file.path(), &path).unwrap();

    // 1. With Skip strategy: all 4 formatted phones must match and be skipped, no new leads inserted
    let skip_options = LeadImportOptions {
        file_path: path.to_string_lossy().to_string(),
        sheet_name: None,
        has_header: true,
        header_row_index: 0,
        mapping: ColumnMapping {
            name_col: Some(0),
            phone_col: Some(1),
            email_col: None,
            company_col: None,
            value_col: None,
            address_col: None,
            about_col: None,
            status_col: None,
            owner_col: None,
        },
        conflict_strategy: ConflictStrategy::Skip,
        default_category_id: None,
        default_status: None,
        default_owner: None,
        default_currency: Some("VND".to_string()),
        normalize_phone: true,
    };

    let summary_skip = execute_lead_import(&mut conn, &skip_options, |_| {}).unwrap();
    assert_eq!(summary_skip.inserted, 0);
    assert_eq!(summary_skip.skipped_duplicates, 4);

    let post_skip_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM leads", [], |r| r.get(0))
        .unwrap();
    assert_eq!(post_skip_count, 4, "Skip must not increase lead count");

    // 2. With Overwrite strategy: all 4 leads must be matched and updated, no new leads inserted
    let overwrite_options = LeadImportOptions {
        conflict_strategy: ConflictStrategy::Overwrite,
        ..skip_options
    };

    let summary_overwrite = execute_lead_import(&mut conn, &overwrite_options, |_| {}).unwrap();
    assert_eq!(summary_overwrite.updated, 4);
    assert_eq!(summary_overwrite.inserted, 0);

    let post_overwrite_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM leads", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        post_overwrite_count, 4,
        "Overwrite must not increase lead count"
    );

    // Verify all 4 names and phones were updated to the incoming values by ID
    let (name1, phone1): (String, String) = conn
        .query_row("SELECT name, phone FROM leads WHERE id = 1", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(name1, "New Name 1");
    assert_eq!(phone1, "0912345678");

    let (name2, phone2): (String, String) = conn
        .query_row("SELECT name, phone FROM leads WHERE id = 2", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(name2, "New Name 2");
    assert_eq!(phone2, "0912345679");

    let (name3, phone3): (String, String) = conn
        .query_row("SELECT name, phone FROM leads WHERE id = 3", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(name3, "New Name 3");
    assert_eq!(phone3, "0912345680");

    let (name4, phone4): (String, String) = conn
        .query_row("SELECT name, phone FROM leads WHERE id = 4", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(name4, "New Name 4");
    assert_eq!(phone4, "0912345681");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_obsolete_identity_keys_cleaned_up_on_overwrite() {
    let mut conn = create_test_db();

    // 1. Seed Alice: id=1, phone=0912345678, email=alice@example.com
    // 2. Seed Charlie & Dave sharing phone 0933333333:
    //    Charlie: id=2, phone=0933333333, email=charlie@example.com
    //    Dave:    id=3, phone=0933333333, email=dave@example.com
    conn.execute(
        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, currency)
         VALUES ('Alice', 'Corp A', 'New', 100.0, '2026-09-09', 'Self', 'alice@example.com', '0912345678', 'VND'),
                ('Charlie', 'Corp C', 'New', 200.0, '2026-09-09', 'Self', 'charlie@example.com', '0933333333', 'VND'),
                ('Dave', 'Corp D', 'New', 300.0, '2026-09-09', 'Self', 'dave@example.com', '0933333333', 'VND')",
        [],
    ).unwrap();

    let mut file = NamedTempFile::new().unwrap();
    // Row 1: Alice updated via email to new phone 0987654321
    // Row 2: Bob imported with Alice's old freed-up phone 0912345678 -> MUST insert Bob as a new lead, NOT overwrite Alice!
    // Row 3: Charlie updated via email to new phone 0944444444 -> Dave's membership on 0933333333 must NOT be deleted!
    // Row 4: Eve imported with Dave's phone 0933333333 -> Under Overwrite, Eve matches Dave and updates Dave!
    let csv_content = "Name,Phone,Email\nAlice Updated,0987654321,alice@example.com\nBob,0912345678,bob@example.com\nCharlie Updated,0944444444,charlie@example.com\nEve Overwriting Dave,0933333333,eve@example.com\n";
    file.write_all(csv_content.as_bytes()).unwrap();

    let path = file.path().with_extension("csv");
    std::fs::copy(file.path(), &path).unwrap();

    let options = LeadImportOptions {
        file_path: path.to_string_lossy().to_string(),
        sheet_name: None,
        has_header: true,
        header_row_index: 0,
        mapping: ColumnMapping {
            name_col: Some(0),
            phone_col: Some(1),
            email_col: Some(2),
            company_col: None,
            value_col: None,
            address_col: None,
            about_col: None,
            status_col: None,
            owner_col: None,
        },
        conflict_strategy: ConflictStrategy::Overwrite,
        default_category_id: None,
        default_status: None,
        default_owner: None,
        default_currency: Some("VND".to_string()),
        normalize_phone: true,
    };

    let summary = execute_lead_import(&mut conn, &options, |_| {}).unwrap();
    // Alice, Charlie, Dave updated (3 updates); Bob inserted (1 insertion)
    assert_eq!(summary.updated, 3);
    assert_eq!(summary.inserted, 1);
    assert_eq!(summary.failed, 0);

    // Verify Alice: id=1 retains her record, updated to phone 0987654321
    let (alice_name, alice_phone, alice_email): (String, String, String) = conn
        .query_row(
            "SELECT name, phone, email FROM leads WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(alice_name, "Alice Updated");
    assert_eq!(alice_phone, "0987654321");
    assert_eq!(alice_email, "alice@example.com");

    // Verify Bob: received a distinct NEW ID (id=4), with phone 0912345678 and email bob@example.com
    let (bob_id, bob_name, bob_phone, bob_email): (i32, String, String, String) = conn
        .query_row(
            "SELECT id, name, phone, email FROM leads WHERE email = 'bob@example.com'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_ne!(
        bob_id, 1,
        "Bob must receive a new distinct ID, not steal Alice's ID"
    );
    assert_eq!(bob_name, "Bob");
    assert_eq!(bob_phone, "0912345678");
    assert_eq!(bob_email, "bob@example.com");

    // Verify Charlie: id=2 updated to phone 0944444444
    let charlie_phone: String = conn
        .query_row("SELECT phone FROM leads WHERE id = 2", [], |r| r.get(0))
        .unwrap();
    assert_eq!(charlie_phone, "0944444444");

    // Verify Dave: id=3 updated by Eve with phone 0933333333
    let (dave_name, dave_phone): (String, String) = conn
        .query_row("SELECT name, phone FROM leads WHERE id = 3", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(dave_name, "Eve Overwriting Dave");
    assert_eq!(dave_phone, "0933333333");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_repeated_updates_clean_up_raw_phone_alias() {
    let mut conn = create_test_db();

    // Seed Alice: id=1, phone=0912345678, email=alice@example.com
    conn.execute(
        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, currency)
         VALUES ('Alice', 'Corp A', 'New', 100.0, '2026-09-09', 'Self', 'alice@example.com', '0912345678', 'VND')",
        [],
    ).unwrap();

    let mut file = NamedTempFile::new().unwrap();
    // Row 1: Alice updated using raw +84912345678 and alice@example.com
    // Row 2: Alice updated again via email to phone 0987654321 -> raw alias +84912345678 must be cleaned from lookup
    // Row 3: Bob imported with +84912345678 and bob@example.com -> must insert Bob as NEW lead, not overwrite Alice!
    let csv_content = "Name,Phone,Email\nAlice Updated 1,+84912345678,alice@example.com\nAlice Updated 2,0987654321,alice@example.com\nBob,+84912345678,bob@example.com\n";
    file.write_all(csv_content.as_bytes()).unwrap();

    let path = file.path().with_extension("csv");
    std::fs::copy(file.path(), &path).unwrap();

    let options = LeadImportOptions {
        file_path: path.to_string_lossy().to_string(),
        sheet_name: None,
        has_header: true,
        header_row_index: 0,
        mapping: ColumnMapping {
            name_col: Some(0),
            phone_col: Some(1),
            email_col: Some(2),
            company_col: None,
            value_col: None,
            address_col: None,
            about_col: None,
            status_col: None,
            owner_col: None,
        },
        conflict_strategy: ConflictStrategy::Overwrite,
        default_category_id: None,
        default_status: None,
        default_owner: None,
        default_currency: Some("VND".to_string()),
        normalize_phone: true,
    };

    let summary = execute_lead_import(&mut conn, &options, |_| {}).unwrap();
    assert_eq!(summary.updated, 2, "Alice must be updated twice");
    assert_eq!(summary.inserted, 1, "Bob must be inserted as a new lead");
    assert_eq!(
        summary.skipped_duplicates, 0,
        "No duplicate must be skipped"
    );
    assert_eq!(summary.failed, 0);

    // Verify Alice: id=1 retains her record, updated to phone 0987654321
    let (alice_name, alice_phone, alice_email): (String, String, String) = conn
        .query_row(
            "SELECT name, phone, email FROM leads WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(alice_name, "Alice Updated 2");
    assert_eq!(alice_phone, "0987654321");
    assert_eq!(alice_email, "alice@example.com");

    // Verify Bob: received a distinct NEW ID, phone 0912345678 (normalized from +84912345678)
    let (bob_id, bob_name, bob_phone, bob_email): (i32, String, String, String) = conn
        .query_row(
            "SELECT id, name, phone, email FROM leads WHERE email = 'bob@example.com'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_ne!(bob_id, 1, "Bob must not steal Alice's record");
    assert_eq!(bob_name, "Bob");
    assert_eq!(bob_phone, "0912345678");
    assert_eq!(bob_email, "bob@example.com");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_repeated_updates_clean_up_domestic_phone_alias() {
    let mut conn = create_test_db();

    // Seed Alice: id=1, phone=0912345678, email=alice@example.com
    conn.execute(
        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, currency)
         VALUES ('Alice', 'Corp A', 'New', 100.0, '2026-09-09', 'Self', 'alice@example.com', '0912345678', 'VND')",
        [],
    ).unwrap();

    let mut file = NamedTempFile::new().unwrap();
    // Row 1: Alice updated using raw +84912345678 and alice@example.com
    // Row 2: Alice updated again via email to phone 0987654321 -> domestic key 0912345678 must be free
    // Row 3: Charlie imported with domestic 0912345678 and charlie@example.com -> must insert Charlie as NEW lead, not skipped as batch duplicate!
    let csv_content = "Name,Phone,Email\nAlice Updated 1,+84912345678,alice@example.com\nAlice Updated 2,0987654321,alice@example.com\nCharlie,0912345678,charlie@example.com\n";
    file.write_all(csv_content.as_bytes()).unwrap();

    let path = file.path().with_extension("csv");
    std::fs::copy(file.path(), &path).unwrap();

    let options = LeadImportOptions {
        file_path: path.to_string_lossy().to_string(),
        sheet_name: None,
        has_header: true,
        header_row_index: 0,
        mapping: ColumnMapping {
            name_col: Some(0),
            phone_col: Some(1),
            email_col: Some(2),
            company_col: None,
            value_col: None,
            address_col: None,
            about_col: None,
            status_col: None,
            owner_col: None,
        },
        conflict_strategy: ConflictStrategy::Overwrite,
        default_category_id: None,
        default_status: None,
        default_owner: None,
        default_currency: Some("VND".to_string()),
        normalize_phone: true,
    };

    let summary = execute_lead_import(&mut conn, &options, |_| {}).unwrap();
    assert_eq!(summary.updated, 2, "Alice must be updated twice");
    assert_eq!(
        summary.inserted, 1,
        "Charlie must be inserted as a new lead"
    );
    assert_eq!(
        summary.skipped_duplicates, 0,
        "No duplicate must be falsely skipped in batch"
    );
    assert_eq!(summary.failed, 0);

    // Verify Alice: id=1 retains her record, updated to phone 0987654321
    let (alice_name, alice_phone, alice_email): (String, String, String) = conn
        .query_row(
            "SELECT name, phone, email FROM leads WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(alice_name, "Alice Updated 2");
    assert_eq!(alice_phone, "0987654321");
    assert_eq!(alice_email, "alice@example.com");

    // Verify Charlie: received a distinct NEW ID, phone 0912345678
    let (charlie_id, charlie_name, charlie_phone, charlie_email): (i32, String, String, String) =
        conn.query_row(
            "SELECT id, name, phone, email FROM leads WHERE email = 'charlie@example.com'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_ne!(charlie_id, 1, "Charlie must not steal Alice's record");
    assert_eq!(charlie_name, "Charlie");
    assert_eq!(charlie_phone, "0912345678");
    assert_eq!(charlie_email, "charlie@example.com");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_inserted_lead_repeated_updates_clean_up_aliases() {
    let mut conn = create_test_db();

    let mut file = NamedTempFile::new().unwrap();
    // Row 1: Zara inserted with +84911111111 and zara@example.com
    // Row 2: Zara updated via email to phone 0988888888 -> old raw alias +84911111111 and domestic 0911111111 must be cleaned
    // Row 3: Zoe imported with raw +84911111111 and zoe@example.com -> MUST be inserted as new lead, NOT overwrite Zara!
    let csv_content = "Name,Phone,Email\nZara,+84911111111,zara@example.com\nZara Updated,0988888888,zara@example.com\nZoe,+84911111111,zoe@example.com\n";
    file.write_all(csv_content.as_bytes()).unwrap();

    let path = file.path().with_extension("csv");
    std::fs::copy(file.path(), &path).unwrap();

    let options = LeadImportOptions {
        file_path: path.to_string_lossy().to_string(),
        sheet_name: None,
        has_header: true,
        header_row_index: 0,
        mapping: ColumnMapping {
            name_col: Some(0),
            phone_col: Some(1),
            email_col: Some(2),
            company_col: None,
            value_col: None,
            address_col: None,
            about_col: None,
            status_col: None,
            owner_col: None,
        },
        conflict_strategy: ConflictStrategy::Overwrite,
        default_category_id: None,
        default_status: None,
        default_owner: None,
        default_currency: Some("VND".to_string()),
        normalize_phone: true,
    };

    let summary = execute_lead_import(&mut conn, &options, |_| {}).unwrap();
    assert_eq!(summary.inserted, 2, "Zara and Zoe must both be inserted");
    assert_eq!(summary.updated, 1, "Zara must be updated once");
    assert_eq!(
        summary.skipped_duplicates, 0,
        "No duplicate must be falsely skipped in batch"
    );
    assert_eq!(summary.failed, 0);

    let (zara_id, zara_name, zara_phone): (i32, String, String) = conn
        .query_row(
            "SELECT id, name, phone FROM leads WHERE email = 'zara@example.com'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(zara_name, "Zara Updated");
    assert_eq!(zara_phone, "0988888888");

    let (zoe_id, zoe_name, zoe_phone): (i32, String, String) = conn
        .query_row(
            "SELECT id, name, phone FROM leads WHERE email = 'zoe@example.com'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_ne!(zoe_id, zara_id, "Zoe must not overwrite Zara");
    assert_eq!(zoe_name, "Zoe");
    assert_eq!(zoe_phone, "0911111111");

    let _ = std::fs::remove_file(&path);
}

fn excel_col_name(mut col_1based: usize) -> String {
    let mut chars = Vec::new();
    while col_1based > 0 {
        let rem = (col_1based - 1) % 26;
        chars.push((b'A' + rem as u8) as char);
        col_1based = (col_1based - 1) / 26;
    }
    chars.reverse();
    chars.into_iter().collect()
}

fn write_test_xlsx(
    dir: &std::path::Path,
    filename: &str,
    sheet_name: &str,
    sheet_xml: &str,
    compression: zip::CompressionMethod,
) -> std::path::PathBuf {
    let path = dir.join(filename);
    let file = std::fs::File::create(&path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(compression)
        .unix_permissions(0o644);

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>"#,
    )
    .unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
    )
    .unwrap();

    zip.start_file("xl/_rels/workbook.xml.rels", options)
        .unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#,
    )
    .unwrap();

    zip.start_file("xl/workbook.xml", options).unwrap();
    let workbook_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="{}" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#,
        sheet_name
    );
    zip.write_all(workbook_xml.as_bytes()).unwrap();

    zip.start_file("xl/worksheets/sheet1.xml", options).unwrap();
    zip.write_all(sheet_xml.as_bytes()).unwrap();
    zip.finish().unwrap();

    path
}

fn default_name_phone_email_options(path: &std::path::Path) -> LeadImportOptions {
    LeadImportOptions {
        file_path: path.to_string_lossy().to_string(),
        sheet_name: None,
        has_header: true,
        header_row_index: 0,
        mapping: ColumnMapping {
            name_col: Some(0),
            phone_col: Some(1),
            email_col: Some(2),
            company_col: None,
            value_col: None,
            address_col: None,
            about_col: None,
            status_col: None,
            owner_col: None,
        },
        conflict_strategy: ConflictStrategy::Skip,
        default_category_id: None,
        default_status: None,
        default_owner: None,
        default_currency: Some("VND".to_string()),
        normalize_phone: true,
    }
}

#[test]
fn test_xlsx_far_out_cell_xfd1048576_rejected_quickly() {
    let dir = tempfile::tempdir().unwrap();
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:XFD1048576"/>
  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>Name</t></is></c>
      <c r="B1" t="inlineStr"><is><t>Phone</t></is></c>
      <c r="C1" t="inlineStr"><is><t>Email</t></is></c>
    </row>
    <row r="2">
      <c r="A2" t="inlineStr"><is><t>Nguyen Van A</t></is></c>
      <c r="B2" t="inlineStr"><is><t>0912345678</t></is></c>
      <c r="C2" t="inlineStr"><is><t>a@example.com</t></is></c>
    </row>
    <row r="3">
      <c r="A3" t="inlineStr"><is><t>Tran Thi B</t></is></c>
      <c r="B3" t="inlineStr"><is><t>0987654321</t></is></c>
      <c r="C3" t="inlineStr"><is><t>b@example.com</t></is></c>
    </row>
    <row r="1048576">
      <c r="XFD1048576" t="inlineStr"><is><t>stray</t></is></c>
    </row>
  </sheetData>
</worksheet>"#;

    let path = write_test_xlsx(
        dir.path(),
        "xfd_bomb.xlsx",
        "Leads",
        sheet_xml,
        zip::CompressionMethod::Deflated,
    );

    let inspect_err = inspect_lead_file(&path, None)
        .expect_err("Inspect must reject sheet with stray cell at XFD1048576");
    assert!(
        inspect_err.contains("Trang tính") && inspect_err.contains("quá lớn"),
        "Unexpected inspect error: {}",
        inspect_err
    );

    let mut conn = create_test_db();
    let options = default_name_phone_email_options(&path);
    let import_err = execute_lead_import(&mut conn, &options, |_| {})
        .expect_err("Import must reject sheet with stray cell at XFD1048576");
    assert!(
        import_err.contains("Trang tính") && import_err.contains("quá lớn"),
        "Unexpected import error: {}",
        import_err
    );

    let lead_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM leads", [], |r| r.get(0))
        .unwrap();
    assert_eq!(lead_count, 0);
}

#[test]
fn test_xlsx_far_out_cell_without_or_lying_dimension_rejected() {
    use super::importer::parse_import_rows;

    let dir = tempfile::tempdir().unwrap();
    let body = r#"  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>Name</t></is></c>
      <c r="B1" t="inlineStr"><is><t>Phone</t></is></c>
      <c r="C1" t="inlineStr"><is><t>Email</t></is></c>
    </row>
    <row r="2">
      <c r="A2" t="inlineStr"><is><t>Nguyen Van A</t></is></c>
      <c r="B2" t="inlineStr"><is><t>0912345678</t></is></c>
      <c r="C2" t="inlineStr"><is><t>a@example.com</t></is></c>
    </row>
    <row r="1048576">
      <c r="XFD1048576" t="inlineStr"><is><t>stray</t></is></c>
    </row>
  </sheetData>"#;

    let cases = [
        (
            "no_dim.xlsx",
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
{}
</worksheet>"#,
                body
            ),
        ),
        (
            "lying_dim.xlsx",
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C3"/>
{}
</worksheet>"#,
                body
            ),
        ),
    ];

    for (filename, xml) in cases {
        let path = write_test_xlsx(
            dir.path(),
            filename,
            "Leads",
            &xml,
            zip::CompressionMethod::Deflated,
        );
        let inspect_err = inspect_lead_file(&path, None)
            .expect_err("Inspect must reject XFD1048576 regardless of dimension tag");
        assert!(
            inspect_err.contains("quá lớn"),
            "{}: {}",
            filename,
            inspect_err
        );

        let options = default_name_phone_email_options(&path);
        let parse_err = parse_import_rows(&options)
            .expect_err("Import parse must reject XFD1048576 regardless of dimension tag");
        assert!(parse_err.contains("quá lớn"), "{}: {}", filename, parse_err);
    }
}

#[test]
fn test_xlsx_row_and_col_limit_boundaries() {
    use super::importer::{parse_import_rows, MAX_IMPORT_COLS, MAX_IMPORT_ROWS};

    let dir = tempfile::tempdir().unwrap();

    // 1. Row span = MAX_IMPORT_ROWS + 1 (within col limit)
    let over_row = MAX_IMPORT_ROWS + 1;
    let over_row_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Name</t></is></c></row>
    <row r="{0}"><c r="A{0}" t="inlineStr"><is><t>Too far</t></is></c></row>
  </sheetData>
</worksheet>"#,
        over_row
    );
    let over_row_path = write_test_xlsx(
        dir.path(),
        "over_rows.xlsx",
        "Leads",
        &over_row_xml,
        zip::CompressionMethod::Stored,
    );
    assert!(inspect_lead_file(&over_row_path, None)
        .unwrap_err()
        .contains("quá lớn"));
    assert!(
        parse_import_rows(&default_name_phone_email_options(&over_row_path))
            .unwrap_err()
            .contains("quá lớn")
    );

    // 2. Column span = MAX_IMPORT_COLS + 1 (within row limit)
    let over_col_letters = excel_col_name(MAX_IMPORT_COLS + 1);
    let over_col_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>Name</t></is></c>
      <c r="{0}1" t="inlineStr"><is><t>OverCol</t></is></c>
    </row>
  </sheetData>
</worksheet>"#,
        over_col_letters
    );
    let over_col_path = write_test_xlsx(
        dir.path(),
        "over_cols.xlsx",
        "Leads",
        &over_col_xml,
        zip::CompressionMethod::Stored,
    );
    assert!(inspect_lead_file(&over_col_path, None)
        .unwrap_err()
        .contains("quá lớn"));
    assert!(
        parse_import_rows(&default_name_phone_email_options(&over_col_path))
            .unwrap_err()
            .contains("quá lớn")
    );

    // 3. Empty formatted cell at XFD1048576 (no value) must be skipped, not fail
    let empty_stray_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:XFD1048576"/>
  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>Name</t></is></c>
      <c r="B1" t="inlineStr"><is><t>Phone</t></is></c>
      <c r="C1" t="inlineStr"><is><t>Email</t></is></c>
    </row>
    <row r="2">
      <c r="A2" t="inlineStr"><is><t>Nguyen Van A</t></is></c>
      <c r="B2" t="inlineStr"><is><t>0912345678</t></is></c>
      <c r="C2" t="inlineStr"><is><t>a@example.com</t></is></c>
    </row>
    <row r="3">
      <c r="A3" t="inlineStr"><is><t>Tran Thi B</t></is></c>
      <c r="B3" t="inlineStr"><is><t>0987654321</t></is></c>
      <c r="C3" t="inlineStr"><is><t>b@example.com</t></is></c>
    </row>
    <row r="1048576">
      <c r="XFD1048576"></c>
    </row>
  </sheetData>
</worksheet>"#;
    let empty_stray_path = write_test_xlsx(
        dir.path(),
        "empty_stray.xlsx",
        "Leads",
        empty_stray_xml,
        zip::CompressionMethod::Deflated,
    );
    let inspection = inspect_lead_file(&empty_stray_path, None)
        .expect("Empty cell at XFD1048576 must be ignored");
    assert_eq!(inspection.headers, vec!["Name", "Phone", "Email"]);
    assert_eq!(inspection.total_rows_estimate, 2);

    let mut conn = create_test_db();
    let summary = execute_lead_import(
        &mut conn,
        &default_name_phone_email_options(&empty_stray_path),
        |_| {},
    )
    .expect("Import must succeed when stray XFD1048576 cell is empty");
    assert_eq!(summary.inserted, 2);
    assert_eq!(summary.failed, 0);
}

#[test]
fn test_xlsx_sparse_offset_origin_and_blank_row() {
    let dir = tempfile::tempdir().unwrap();
    // Sheet starts at B2 (row 2, col B), row 4 is blank, row 5 has invalid phone.
    let sheet_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="2">
      <c r="B2" t="inlineStr"><is><t>Name</t></is></c>
      <c r="C2" t="inlineStr"><is><t>Phone</t></is></c>
      <c r="D2" t="inlineStr"><is><t>Email</t></is></c>
    </row>
    <row r="3">
      <c r="B3" t="inlineStr"><is><t>Valid Lead</t></is></c>
      <c r="C3" t="inlineStr"><is><t>0912345678</t></is></c>
      <c r="D3" t="inlineStr"><is><t>valid@example.com</t></is></c>
    </row>
    <row r="5">
      <c r="B5" t="inlineStr"><is><t>Bad Phone</t></is></c>
      <c r="C5" t="inlineStr"><is><t>12345</t></is></c>
    </row>
  </sheetData>
</worksheet>"#;

    let path = write_test_xlsx(
        dir.path(),
        "offset_origin.xlsx",
        "Leads",
        sheet_xml,
        zip::CompressionMethod::Deflated,
    );

    let inspection = inspect_lead_file(&path, None).expect("Offset origin inspect should succeed");
    assert_eq!(inspection.headers, vec!["Name", "Phone", "Email"]);
    assert_eq!(inspection.total_rows_estimate, 3);
    assert_eq!(inspection.sample_rows.len(), 3);
    assert_eq!(inspection.sample_rows[1], vec!["", "", ""]);

    let mut conn = create_test_db();
    let summary =
        execute_lead_import(&mut conn, &default_name_phone_email_options(&path), |_| {}).unwrap();
    assert_eq!(summary.inserted, 1);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.error_rows.len(), 1);
    assert_eq!(summary.error_rows[0].row_number, 5);
}

#[test]
fn test_xlsx_100k_rows_inspect_and_parse_succeed() {
    use super::importer::parse_import_rows;
    use std::fmt::Write as _;

    const DATA_ROWS: usize = 100_000;

    let dir = tempfile::tempdir().unwrap();
    let mut sheet_xml = String::with_capacity(DATA_ROWS * 115 + 512);
    sheet_xml.push_str(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Name</t></is></c><c r="B1" t="inlineStr"><is><t>Phone</t></is></c></row>
"#,
    );
    for i in 1..=DATA_ROWS {
        let row_num = i + 1;
        let _ = writeln!(
            sheet_xml,
            r#"    <row r="{r}"><c r="A{r}" t="inlineStr"><is><t>Lead {i}</t></is></c><c r="B{r}" t="inlineStr"><is><t>0912345678</t></is></c></row>"#,
            r = row_num,
            i = i
        );
    }
    sheet_xml.push_str("  </sheetData>\n</worksheet>");

    let path = write_test_xlsx(
        dir.path(),
        "leads_100k.xlsx",
        "Leads",
        &sheet_xml,
        zip::CompressionMethod::Stored,
    );

    let inspection = inspect_lead_file(&path, None).expect("100k-row inspect must succeed");
    assert_eq!(inspection.headers, vec!["Name", "Phone"]);
    assert_eq!(inspection.sample_rows.len(), 10);
    assert_eq!(inspection.total_rows_estimate, DATA_ROWS);

    let options = default_name_phone_email_options(&path);
    let rows = parse_import_rows(&options).expect("100k-row parse must succeed");
    assert_eq!(rows.len(), DATA_ROWS + 1);
    assert_eq!(rows[0].source_row_number, 1);
    assert_eq!(rows[DATA_ROWS].source_row_number, DATA_ROWS + 1);
    assert_eq!(
        rows[DATA_ROWS].cells,
        vec!["Lead 100000".to_string(), "0912345678".to_string()]
    );
}
