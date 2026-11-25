use calamine::Data;
use std::path::Path;

use crate::lead_importer::sheet_reader::inspect_excel_sheet;
use crate::lead_importer::types::ImportInspection;

pub fn inspect_lead_file(
    file_path: &Path,
    sheet_name: Option<&str>,
) -> Result<ImportInspection, String> {
    if !file_path.exists() {
        return Err(format!("Tệp không tồn tại: {}", file_path.display()));
    }

    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "csv" => inspect_csv(file_path),
        "xlsx" | "xls" | "xlsb" | "ods" => inspect_excel(file_path, sheet_name),
        _ => Err(format!(
            "Định dạng tệp không được hỗ trợ: .{} (Chỉ hỗ trợ .xlsx, .xls, .csv)",
            ext
        )),
    }
}

fn inspect_csv(file_path: &Path) -> Result<ImportInspection, String> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_path(file_path)
        .map_err(|e| format!("Không thể mở tệp CSV: {}", e))?;

    let mut all_rows: Vec<Vec<String>> = Vec::new();
    let mut row_count: usize = 0;

    for result in reader.records() {
        let record = result.map_err(|e| format!("Lỗi đọc dòng CSV {}: {}", row_count + 1, e))?;
        row_count += 1;
        if all_rows.len() < 11 {
            let row = record.iter().map(|s| s.trim().to_string()).collect();
            all_rows.push(row);
        }
    }

    if all_rows.is_empty() {
        return Err("Tệp CSV trống hoặc không có dữ liệu".to_string());
    }

    let headers = all_rows[0].clone();
    let sample_rows = if all_rows.len() > 1 {
        all_rows[1..].to_vec()
    } else {
        Vec::new()
    };

    Ok(ImportInspection {
        file_type: "csv".to_string(),
        sheets: vec!["Default".to_string()],
        selected_sheet: Some("Default".to_string()),
        headers,
        sample_rows,
        total_rows_estimate: row_count.saturating_sub(1),
    })
}

fn inspect_excel(
    file_path: &Path,
    requested_sheet: Option<&str>,
) -> Result<ImportInspection, String> {
    inspect_excel_sheet(file_path, requested_sheet)
}

pub fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.trim().to_string(),
        Data::Float(f) => {
            if f.fract() == 0.0 && *f >= -1e15 && *f <= 1e15 {
                format!("{:.0}", f)
            } else {
                format!("{}", f)
            }
        }
        Data::Int(i) => format!("{}", i),
        Data::Bool(b) => format!("{}", b),
        Data::DateTime(d) => format!("{:.2}", d),
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
        Data::Error(_) => String::new(),
    }
}
