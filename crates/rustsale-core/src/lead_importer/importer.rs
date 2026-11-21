use calamine::{open_workbook_auto, Reader, Sheets};
use rusqlite::{params, Connection, TransactionBehavior};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::lead_importer::inspector::cell_to_string;
use crate::lead_importer::types::{
    ConflictStrategy, ImportErrorRow, ImportProgress, ImportSummaryResult, LeadImportOptions,
    SourceRow,
};

pub fn normalize_vn_mobile(value: &str) -> Option<String> {
    let raw_trimmed = value.trim();
    let has_plus = raw_trimmed.starts_with('+');
    let digits: String = raw_trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }

    let normalized = if let Some(rest) = digits.strip_prefix("0084") {
        if let Some(without_zero) = rest.strip_prefix('0') {
            format!("0{without_zero}")
        } else {
            format!("0{rest}")
        }
    } else if has_plus && digits.starts_with("84") {
        let rest = &digits[2..];
        if let Some(without_zero) = rest.strip_prefix('0') {
            format!("0{without_zero}")
        } else {
            format!("0{rest}")
        }
    } else if digits.len() == 9
        && ["3", "5", "7", "8", "9"]
            .iter()
            .any(|prefix| digits.starts_with(prefix))
    {
        // 9-digit domestic number where Excel truncated leading zero (e.g. 841234567 -> 0841234567, 912345678 -> 0912345678)
        format!("0{}", digits)
    } else if (digits.len() == 11 || digits.len() == 12) && digits.starts_with("84") {
        // 11 digits: 84 + 9-digit mobile (e.g. 84841234567 -> 0841234567, 84912345678 -> 0912345678)
        // 12 digits: 84 + 0 + 9-digit mobile (e.g. 840912345678 -> 0912345678)
        let rest = &digits[2..];
        if let Some(without_zero) = rest.strip_prefix('0') {
            format!("0{without_zero}")
        } else {
            format!("0{rest}")
        }
    } else {
        digits
    };

    if normalized.len() == 10
        && ["03", "05", "07", "08", "09"]
            .iter()
            .any(|prefix| normalized.starts_with(prefix))
    {
        Some(normalized)
    } else {
        None
    }
}

pub fn normalize_email(value: &str) -> Option<String> {
    let trimmed = value.trim().to_lowercase();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.contains('@') && trimmed.contains('.') && trimmed.len() >= 5 {
        Some(trimmed)
    } else {
        None
    }
}

pub fn phone_lookup_keys(phone: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let trimmed = phone.trim();
    if trimmed.is_empty() {
        return keys;
    }
    if let Some(norm) = normalize_vn_mobile(trimmed) {
        keys.push(norm.clone());
        let intl_plus = format!("+84{}", &norm[1..]);
        if !keys.contains(&intl_plus) {
            keys.push(intl_plus);
        }
        let intl_digits = format!("84{}", &norm[1..]);
        if !keys.contains(&intl_digits) {
            keys.push(intl_digits);
        }
    }
    if !keys.contains(&trimmed.to_string()) {
        keys.push(trimmed.to_string());
    }
    let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
    if !digits.is_empty() && !keys.contains(&digits) {
        keys.push(digits);
    }
    keys
}

pub fn combined_phone_lookup_keys(final_phone: &str, raw_phone: &str) -> Vec<String> {
    let mut keys = phone_lookup_keys(final_phone);
    for k in phone_lookup_keys(raw_phone) {
        if !keys.contains(&k) {
            keys.push(k);
        }
    }
    keys
}

fn validate_thousands_grouping(s: &str, sep: char) -> bool {
    if s.is_empty() || s.starts_with(sep) || s.ends_with(sep) {
        return false;
    }
    let parts: Vec<&str> = s.split(sep).collect();
    if parts.len() <= 1 {
        return s.chars().all(|c| c.is_ascii_digit());
    }
    let first = parts[0];
    if first.is_empty() || first.len() > 3 || !first.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    for &part in &parts[1..] {
        if part.len() != 3 || !part.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
    }
    true
}

pub fn parse_lead_monetary_value(
    value_str: &str,
    expected_currency: &str,
) -> Result<Option<f64>, String> {
    let trimmed = value_str.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let is_usd = expected_currency.eq_ignore_ascii_case("USD");

    // Validate currency markers against the selected import denomination (F1)
    if is_usd {
        if trimmed.contains('₫')
            || trimmed.contains('đ')
            || trimmed.contains('Đ')
            || trimmed.to_uppercase().contains("VND")
        {
            return Err(format!(
                "Giá trị giao dịch '{}' chứa ký hiệu tiền tệ VND trong khi đơn vị nhập là USD",
                trimmed
            ));
        }
    } else {
        // Expected currency is VND
        if trimmed.contains('$') || trimmed.to_uppercase().contains("USD") {
            return Err(format!(
                "Giá trị giao dịch '{}' chứa ký hiệu ngoại tệ USD trong khi đơn vị nhập là VND",
                trimmed
            ));
        }
    }

    // Strip currency symbols and units
    let mut cleaned = if is_usd {
        trimmed
            .replace("USD", "")
            .replace("usd", "")
            .replace('$', "")
            .trim()
            .to_string()
    } else {
        trimmed
            .replace("VND", "")
            .replace("vnd", "")
            .replace(['₫', 'đ', 'Đ'], "")
            .trim()
            .to_string()
    };

    if cleaned.is_empty() {
        return Ok(None);
    }

    // Handle space separators if used as thousands grouping (e.g. "1 000 000")
    if cleaned.contains(' ') {
        if !validate_thousands_grouping(&cleaned, ' ') {
            return Err(format!(
                "Giá trị giao dịch '{}' không đúng định dạng phân cách hàng nghìn",
                trimmed
            ));
        }
        cleaned = cleaned.replace(' ', "");
    }

    // Reject consecutive separators or invalid mixed sequences (e.g. 1..000, 1,,000, 1.,000)
    if cleaned.contains("..")
        || cleaned.contains(",,")
        || cleaned.contains(".,")
        || cleaned.contains(",.")
    {
        return Err(format!(
            "Giá trị giao dịch '{}' chứa ký tự phân cách lặp hoặc không hợp lệ",
            trimmed
        ));
    }

    // Check if both dots and commas exist
    let has_dot = cleaned.contains('.');
    let has_comma = cleaned.contains(',');

    if let (true, true, Some(last_dot), Some(last_comma)) =
        (has_dot, has_comma, cleaned.rfind('.'), cleaned.rfind(','))
    {
        if last_comma > last_dot {
            // Vietnamese format: 1.000.000,50 -> dot is thousands separator, comma is decimal
            let int_part = &cleaned[..last_comma];
            let dec_part = &cleaned[last_comma + 1..];
            if !validate_thousands_grouping(int_part, '.')
                || dec_part.is_empty()
                || !dec_part.chars().all(|c| c.is_ascii_digit())
            {
                return Err(format!(
                    "Giá trị giao dịch '{}' không đúng định dạng nhóm hàng nghìn hoặc số thập phân",
                    trimmed
                ));
            }
            cleaned = format!("{}.{}", int_part.replace('.', ""), dec_part);
        } else {
            // US format: 1,000,000.50 -> comma is thousands separator, dot is decimal
            let int_part = &cleaned[..last_dot];
            let dec_part = &cleaned[last_dot + 1..];
            if !validate_thousands_grouping(int_part, ',')
                || dec_part.is_empty()
                || !dec_part.chars().all(|c| c.is_ascii_digit())
            {
                return Err(format!(
                    "Giá trị giao dịch '{}' không đúng định dạng nhóm hàng nghìn hoặc số thập phân",
                    trimmed
                ));
            }
            cleaned = format!("{}.{}", int_part.replace(',', ""), dec_part);
        }
    } else if has_comma {
        // Only commas
        let comma_count = cleaned.matches(',').count();
        if comma_count > 1 {
            // E.g. 1,000,000 -> thousand separators
            if !validate_thousands_grouping(&cleaned, ',') {
                return Err(format!(
                    "Giá trị giao dịch '{}' có phân nhóm hàng nghìn bằng dấu phẩy không hợp lệ",
                    trimmed
                ));
            }
            cleaned = cleaned.replace(',', "");
        } else {
            // Single comma: e.g. 50,5 or 50,000
            let Some(comma_idx) = cleaned.find(',') else {
                return Err(format!(
                    "Giá trị giao dịch '{}' không đúng định dạng số",
                    trimmed
                ));
            };
            let int_part = &cleaned[..comma_idx];
            let dec_part = &cleaned[comma_idx + 1..];
            if int_part.is_empty()
                || dec_part.is_empty()
                || !int_part.chars().all(|c| c.is_ascii_digit())
                || !dec_part.chars().all(|c| c.is_ascii_digit())
            {
                return Err(format!(
                    "Giá trị giao dịch '{}' không đúng định dạng số",
                    trimmed
                ));
            }
            if dec_part.len() == 3 && int_part.len() <= 3 {
                // E.g. 50,000 -> thousands separator
                cleaned = cleaned.replace(',', "");
            } else {
                // E.g. 50,5 or 12,34 -> decimal separator
                cleaned = format!("{}.{}", int_part, dec_part);
            }
        }
    } else if has_dot {
        // Only dots
        let dot_count = cleaned.matches('.').count();
        if dot_count > 1 {
            // E.g. 1.000.000 -> thousand separators
            if !validate_thousands_grouping(&cleaned, '.') {
                return Err(format!(
                    "Giá trị giao dịch '{}' có phân nhóm hàng nghìn bằng dấu chấm không hợp lệ",
                    trimmed
                ));
            }
            cleaned = cleaned.replace('.', "");
        } else {
            // Single dot: e.g. 50.000 or 50.5
            let Some(dot_idx) = cleaned.find('.') else {
                return Err(format!(
                    "Giá trị giao dịch '{}' không đúng định dạng số",
                    trimmed
                ));
            };
            let int_part = &cleaned[..dot_idx];
            let dec_part = &cleaned[dot_idx + 1..];
            if int_part.is_empty()
                || dec_part.is_empty()
                || !int_part.chars().all(|c| c.is_ascii_digit())
                || !dec_part.chars().all(|c| c.is_ascii_digit())
            {
                return Err(format!(
                    "Giá trị giao dịch '{}' không đúng định dạng số",
                    trimmed
                ));
            }
            if dec_part.len() == 3 && int_part.len() <= 3 {
                // E.g. 50.000 (common in VN VND) -> thousand separator
                cleaned = cleaned.replace('.', "");
            }
            // Otherwise e.g. 50.5 or 12.34 -> keep as decimal point
        }
    } else if !cleaned.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!(
            "Giá trị giao dịch '{}' không đúng định dạng số hợp lệ",
            trimmed
        ));
    }

    match cleaned.parse::<f64>() {
        Ok(v) if v >= 0.0 && v.is_finite() => Ok(Some(v)),
        _ => Err(format!(
            "Giá trị giao dịch '{}' không đúng định dạng số hợp lệ",
            trimmed
        )),
    }
}

pub fn parse_lead_value(value_str: &str, expected_currency: &str) -> f64 {
    parse_lead_monetary_value(value_str, expected_currency)
        .unwrap_or(None)
        .unwrap_or(0.0)
}

pub fn parse_import_rows(options: &LeadImportOptions) -> Result<Vec<SourceRow>, String> {
    let file_path = Path::new(&options.file_path);
    if !file_path.exists() {
        return Err(format!("Tệp không tồn tại: {}", file_path.display()));
    }

    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "csv" => read_all_csv_rows(file_path),
        "xlsx" | "xls" | "xlsb" | "ods" => {
            read_all_excel_rows(file_path, options.sheet_name.as_deref())
        }
        _ => Err(format!(
            "Định dạng tệp không được hỗ trợ: .{} (Chỉ hỗ trợ .xlsx, .xls, .csv)",
            ext
        )),
    }
}

pub fn execute_lead_import_rows<F>(
    connection: &mut Connection,
    options: &LeadImportOptions,
    all_data_rows: Vec<SourceRow>,
    mut progress_callback: F,
) -> Result<ImportSummaryResult, String>
where
    F: FnMut(ImportProgress),
{
    if all_data_rows.is_empty() {
        return Ok(ImportSummaryResult::default());
    }

    let start_row_idx = if options.has_header {
        options.header_row_index + 1
    } else {
        0
    };

    let data_rows = if start_row_idx < all_data_rows.len() {
        &all_data_rows[start_row_idx..]
    } else {
        &[]
    };

    let total_rows = data_rows.len();
    let mut summary = ImportSummaryResult {
        total_rows,
        ..Default::default()
    };

    if total_rows == 0 {
        return Ok(summary);
    }

    let tx = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| format!("Lỗi mở transaction SQLite: {}", e))?;

    // Validate category exists if specified (R5)
    if let Some(cat_id) = options.default_category_id {
        let cat_exists: bool = tx
            .query_row(
                "SELECT 1 FROM categories WHERE id = ?1 LIMIT 1",
                params![cat_id],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if !cat_exists {
            return Err(format!(
                "Danh mục với ID {} không tồn tại hoặc đã bị xóa.",
                cat_id
            ));
        }
    }

    let now = chrono::Utc::now().to_rfc3339();

    let default_status = options
        .default_status
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("New");
    let default_owner = options
        .default_owner
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Self");
    let default_currency_raw = options
        .default_currency
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("VND")
        .trim()
        .to_uppercase();

    if default_currency_raw != "VND" && default_currency_raw != "USD" {
        return Err(format!(
            "Đơn vị tiền tệ mặc định '{}' không hợp lệ: chỉ hỗ trợ VND hoặc USD.",
            default_currency_raw
        ));
    }
    let default_currency = default_currency_raw.as_str();

    let mut phone_to_lead_ids: HashMap<String, HashSet<i32>> = HashMap::new();
    let mut email_to_lead_ids: HashMap<String, HashSet<i32>> = HashMap::new();
    let mut lead_current_identity: HashMap<i32, (Vec<String>, Option<String>)> = HashMap::new();

    {
        let mut load_existing_stmt = tx
            .prepare("SELECT id, phone, email FROM leads WHERE (phone IS NOT NULL AND phone != '') OR (email IS NOT NULL AND email != '')")
            .map_err(|e| format!("Lỗi prepare load existing leads: {}", e))?;

        let rows = load_existing_stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i32>(0)?,
                    r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                ))
            })
            .map_err(|e| format!("Lỗi truy vấn leads hiện có: {}", e))?;

        for row_res in rows {
            let (id, phone, email) = row_res.map_err(|e| format!("Lỗi đọc lead hiện có: {}", e))?;

            let p_keys = phone_lookup_keys(&phone);
            for k in &p_keys {
                phone_to_lead_ids.entry(k.clone()).or_default().insert(id);
            }

            let trimmed_email = email.trim();
            let norm_email = if !trimmed_email.is_empty() {
                normalize_email(trimmed_email).or_else(|| {
                    let lower = trimmed_email.to_lowercase();
                    if !lower.is_empty() {
                        Some(lower)
                    } else {
                        None
                    }
                })
            } else {
                None
            };

            if let Some(ref e) = norm_email {
                email_to_lead_ids.entry(e.clone()).or_default().insert(id);
            }

            lead_current_identity.insert(id, (p_keys, norm_email));
        }
    }

    {
        let mut insert_lead_stmt = tx
            .prepare(
                "INSERT INTO leads (
                    name, company, status, value, last_activity, owner, email, phone, about, address, currency
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )
            .map_err(|e| format!("Lỗi prepare insert lead: {}", e))?;

        let mut update_lead_stmt = tx
            .prepare(
                "UPDATE leads SET
                    name = CASE WHEN ?1 != '' THEN ?1 ELSE name END,
                    company = CASE WHEN ?2 != '' THEN ?2 ELSE company END,
                    status = CASE WHEN ?3 != '' THEN ?3 ELSE status END,
                    value = CASE WHEN ?4 IS NOT NULL THEN ?4 ELSE value END,
                    last_activity = ?5,
                    owner = CASE WHEN ?6 != '' THEN ?6 ELSE owner END,
                    email = CASE WHEN ?7 != '' THEN ?7 ELSE email END,
                    phone = CASE WHEN ?8 != '' THEN ?8 ELSE phone END,
                    about = CASE WHEN ?9 != '' THEN ?9 ELSE about END,
                    address = CASE WHEN ?10 != '' THEN ?10 ELSE address END,
                    currency = CASE WHEN ?4 IS NOT NULL THEN ?12 ELSE currency END
                WHERE id = ?11",
            )
            .map_err(|e| format!("Lỗi prepare update lead: {}", e))?;

        let mut assign_category_stmt = tx
            .prepare("INSERT OR IGNORE INTO lead_categories (lead_id, category_id) VALUES (?1, ?2)")
            .map_err(|e| format!("Lỗi prepare assign category: {}", e))?;

        let mut processed_count = 0;

        for src_row in data_rows {
            let row_number = src_row.source_row_number;
            let row = &src_row.cells;
            processed_count += 1;

            let mut emit_progress = |count: usize| {
                if count.is_multiple_of(50) || count == total_rows {
                    let percentage = ((count as f64 / total_rows as f64) * 100.0).min(99.0) as u8;
                    progress_callback(ImportProgress {
                        processed_rows: count,
                        total_rows,
                        percentage,
                    });
                }
            };

            // Check if entire row is empty
            if row.iter().all(|c| c.trim().is_empty()) {
                emit_progress(processed_count);
                continue;
            }

            let name = get_mapped_val(row, options.mapping.name_col);
            let raw_phone = get_mapped_val(row, options.mapping.phone_col);
            let raw_email = get_mapped_val(row, options.mapping.email_col);
            let company = get_mapped_val(row, options.mapping.company_col);
            let raw_value = get_mapped_val(row, options.mapping.value_col);
            let address = get_mapped_val(row, options.mapping.address_col);
            let about = get_mapped_val(row, options.mapping.about_col);
            let row_status = get_mapped_val(row, options.mapping.status_col);
            let row_owner = get_mapped_val(row, options.mapping.owner_col);

            // Validation: Name is required
            if name.trim().is_empty() {
                summary.failed += 1;
                summary.error_rows.push(ImportErrorRow {
                    row_number,
                    raw_data: row.clone(),
                    reason: "Họ và tên không được để trống".to_string(),
                });
                emit_progress(processed_count);
                continue;
            }

            // Normalization
            let final_phone = if options.normalize_phone && !raw_phone.trim().is_empty() {
                match normalize_vn_mobile(&raw_phone) {
                    Some(valid) => valid,
                    None => {
                        // If phone is invalid and no valid email either, quarantine row
                        if normalize_email(&raw_email).is_none() {
                            summary.failed += 1;
                            summary.error_rows.push(ImportErrorRow {
                                row_number,
                                raw_data: row.clone(),
                                reason: format!(
                                    "Số điện thoại '{}' không đúng định dạng di động VN (10 số, đầu 03/05/07/08/09)",
                                    raw_phone
                                ),
                            });
                            emit_progress(processed_count);
                            continue;
                        }
                        raw_phone.trim().to_string()
                    }
                }
            } else {
                raw_phone.trim().to_string()
            };

            let final_email =
                normalize_email(&raw_email).unwrap_or_else(|| raw_email.trim().to_string());
            let parsed_monetary_value =
                match parse_lead_monetary_value(&raw_value, default_currency) {
                    Ok(val) => val,
                    Err(err) => {
                        summary.failed += 1;
                        summary.error_rows.push(ImportErrorRow {
                            row_number,
                            raw_data: row.clone(),
                            reason: err,
                        });
                        emit_progress(processed_count);
                        continue;
                    }
                };
            let final_value = parsed_monetary_value.unwrap_or(0.0);

            // Deduplication check
            let norm_email = if !final_email.is_empty() {
                normalize_email(&final_email).unwrap_or_else(|| final_email.trim().to_lowercase())
            } else {
                String::new()
            };

            let all_phone_keys = combined_phone_lookup_keys(&final_phone, &raw_phone);

            let mut existing_lead_id: Option<i32> = None;
            if !all_phone_keys.is_empty() {
                existing_lead_id = all_phone_keys.iter().find_map(|k| {
                    phone_to_lead_ids
                        .get(k)
                        .and_then(|set| set.iter().copied().next())
                });
            }

            if existing_lead_id.is_none() && !norm_email.is_empty() {
                existing_lead_id = email_to_lead_ids
                    .get(&norm_email)
                    .and_then(|set| set.iter().copied().next());
            }

            let is_duplicate = existing_lead_id.is_some();

            match options.conflict_strategy {
                ConflictStrategy::Skip => {
                    if is_duplicate {
                        summary.skipped_duplicates += 1;
                        emit_progress(processed_count);
                        continue;
                    }
                    // Insert
                    let final_status = if !row_status.trim().is_empty() {
                        row_status.trim()
                    } else {
                        default_status
                    };
                    let final_owner = if !row_owner.trim().is_empty() {
                        row_owner.trim()
                    } else {
                        default_owner
                    };
                    insert_lead_stmt
                        .execute(params![
                            name,
                            company,
                            final_status,
                            final_value,
                            now,
                            final_owner,
                            final_email,
                            final_phone,
                            about,
                            address,
                            default_currency,
                        ])
                        .map_err(|e| format!("Lỗi chèn CSDL: {}", e))?;

                    let lead_id = tx.last_insert_rowid() as i32;
                    if let Some(cat_id) = options.default_category_id {
                        assign_category_stmt
                            .execute(params![lead_id, cat_id])
                            .map_err(|e| format!("Lỗi gán danh mục: {}", e))?;
                    }
                    for k in &all_phone_keys {
                        phone_to_lead_ids
                            .entry(k.clone())
                            .or_default()
                            .insert(lead_id);
                    }
                    if !norm_email.is_empty() {
                        email_to_lead_ids
                            .entry(norm_email.clone())
                            .or_default()
                            .insert(lead_id);
                    }
                    lead_current_identity.insert(
                        lead_id,
                        (
                            all_phone_keys,
                            if !norm_email.is_empty() {
                                Some(norm_email.clone())
                            } else {
                                None
                            },
                        ),
                    );
                    summary.inserted += 1;
                }
                ConflictStrategy::Overwrite => {
                    if let Some(lead_id) = existing_lead_id {
                        let overwrite_status = if options.mapping.status_col.is_some()
                            && !row_status.trim().is_empty()
                        {
                            row_status.trim()
                        } else {
                            ""
                        };
                        let overwrite_owner = if options.mapping.owner_col.is_some()
                            && !row_owner.trim().is_empty()
                        {
                            row_owner.trim()
                        } else {
                            ""
                        };
                        let overwrite_value: Option<f64> = if options.mapping.value_col.is_some()
                            && !raw_value.trim().is_empty()
                        {
                            parsed_monetary_value
                        } else {
                            None
                        };

                        update_lead_stmt
                            .execute(params![
                                name,
                                company,
                                overwrite_status,
                                overwrite_value,
                                now,
                                overwrite_owner,
                                final_email,
                                final_phone,
                                about,
                                address,
                                lead_id,
                                default_currency,
                            ])
                            .map_err(|e| format!("Lỗi cập nhật CSDL: {}", e))?;

                        if let Some(cat_id) = options.default_category_id {
                            assign_category_stmt
                                .execute(params![lead_id, cat_id])
                                .map_err(|e| format!("Lỗi gán danh mục: {}", e))?;
                        }

                        let (mut current_phone_keys, mut current_email) = lead_current_identity
                            .remove(&lead_id)
                            .unwrap_or_else(|| (Vec::new(), None));

                        // F4: If a non-blank phone was supplied, remove lead_id from obsolete phone keys and index new ones
                        if !final_phone.is_empty() {
                            for old_k in &current_phone_keys {
                                if let Some(set) = phone_to_lead_ids.get_mut(old_k) {
                                    set.remove(&lead_id);
                                    if set.is_empty() {
                                        phone_to_lead_ids.remove(old_k);
                                    }
                                }
                            }
                            for k in &all_phone_keys {
                                phone_to_lead_ids
                                    .entry(k.clone())
                                    .or_default()
                                    .insert(lead_id);
                            }
                            current_phone_keys = all_phone_keys;
                        }

                        // F4: If a non-blank email was supplied, remove lead_id from obsolete email keys and index new ones
                        if !norm_email.is_empty() {
                            if let Some(ref old_e) = current_email {
                                if let Some(set) = email_to_lead_ids.get_mut(old_e) {
                                    set.remove(&lead_id);
                                    if set.is_empty() {
                                        email_to_lead_ids.remove(old_e);
                                    }
                                }
                            }
                            email_to_lead_ids
                                .entry(norm_email.clone())
                                .or_default()
                                .insert(lead_id);
                            current_email = Some(norm_email.clone());
                        }

                        lead_current_identity.insert(lead_id, (current_phone_keys, current_email));
                        summary.updated += 1;
                    } else {
                        // Insert new
                        let final_status = if !row_status.trim().is_empty() {
                            row_status.trim()
                        } else {
                            default_status
                        };
                        let final_owner = if !row_owner.trim().is_empty() {
                            row_owner.trim()
                        } else {
                            default_owner
                        };
                        insert_lead_stmt
                            .execute(params![
                                name,
                                company,
                                final_status,
                                final_value,
                                now,
                                final_owner,
                                final_email,
                                final_phone,
                                about,
                                address,
                                default_currency,
                            ])
                            .map_err(|e| format!("Lỗi chèn CSDL: {}", e))?;

                        let lead_id = tx.last_insert_rowid() as i32;
                        if let Some(cat_id) = options.default_category_id {
                            assign_category_stmt
                                .execute(params![lead_id, cat_id])
                                .map_err(|e| format!("Lỗi gán danh mục: {}", e))?;
                        }
                        for k in &all_phone_keys {
                            phone_to_lead_ids
                                .entry(k.clone())
                                .or_default()
                                .insert(lead_id);
                        }
                        if !norm_email.is_empty() {
                            email_to_lead_ids
                                .entry(norm_email.clone())
                                .or_default()
                                .insert(lead_id);
                        }
                        lead_current_identity.insert(
                            lead_id,
                            (
                                all_phone_keys,
                                if !norm_email.is_empty() {
                                    Some(norm_email.clone())
                                } else {
                                    None
                                },
                            ),
                        );
                        summary.inserted += 1;
                    }
                }
                ConflictStrategy::Allow => {
                    let final_status = if !row_status.trim().is_empty() {
                        row_status.trim()
                    } else {
                        default_status
                    };
                    let final_owner = if !row_owner.trim().is_empty() {
                        row_owner.trim()
                    } else {
                        default_owner
                    };
                    insert_lead_stmt
                        .execute(params![
                            name,
                            company,
                            final_status,
                            final_value,
                            now,
                            final_owner,
                            final_email,
                            final_phone,
                            about,
                            address,
                            default_currency,
                        ])
                        .map_err(|e| format!("Lỗi chèn CSDL: {}", e))?;

                    let lead_id = tx.last_insert_rowid() as i32;
                    if let Some(cat_id) = options.default_category_id {
                        assign_category_stmt
                            .execute(params![lead_id, cat_id])
                            .map_err(|e| format!("Lỗi gán danh mục: {}", e))?;
                    }
                    summary.inserted += 1;
                }
            }

            emit_progress(processed_count);
        }
    }

    // Commit SQLite transaction first before emitting final 100% progress
    tx.commit()
        .map_err(|e| format!("Lỗi commit transaction SQLite: {}", e))?;

    progress_callback(ImportProgress {
        processed_rows: summary.total_rows,
        total_rows: summary.total_rows,
        percentage: 100,
    });

    Ok(summary)
}

pub fn execute_lead_import<F>(
    connection: &mut Connection,
    options: &LeadImportOptions,
    progress_callback: F,
) -> Result<ImportSummaryResult, String>
where
    F: FnMut(ImportProgress),
{
    let all_data_rows = parse_import_rows(options)?;
    execute_lead_import_rows(connection, options, all_data_rows, progress_callback)
}

fn get_mapped_val(row: &[String], col_idx: Option<usize>) -> String {
    match col_idx {
        Some(idx) if idx < row.len() => row[idx].trim().to_string(),
        _ => String::new(),
    }
}

fn read_all_csv_rows(file_path: &Path) -> Result<Vec<SourceRow>, String> {
    let file_bytes =
        std::fs::read(file_path).map_err(|e| format!("Không thể mở tệp CSV: {}", e))?;

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(&file_bytes[..]);

    let mut rows = Vec::new();
    let mut record = csv::StringRecord::new();
    let mut record_idx = 0;
    while reader
        .read_record(&mut record)
        .map_err(|e| format!("Lỗi đọc dòng CSV: {}", e))?
    {
        record_idx += 1;
        let base_line = record
            .position()
            .map(|p| p.line() as usize)
            .unwrap_or(record_idx);
        let start_byte = record.position().map(|p| p.byte() as usize).unwrap_or(0);

        // Account for any empty lines skipped by csv-core before the start of this record
        let mut skipped_empty_lines = 0;
        if start_byte < file_bytes.len() {
            for &b in &file_bytes[start_byte..] {
                if b == b'\n' {
                    skipped_empty_lines += 1;
                } else if b == b'\r' {
                    continue;
                } else {
                    break;
                }
            }
        }

        let source_row_number = base_line + skipped_empty_lines;
        let mut row_vec: Vec<String> = record.iter().map(|s| s.trim().to_string()).collect();
        if record_idx == 1 && !row_vec.is_empty() && row_vec[0].starts_with('\u{FEFF}') {
            row_vec[0] = row_vec[0].trim_start_matches('\u{FEFF}').to_string();
        }
        rows.push(SourceRow {
            source_row_number,
            cells: row_vec,
        });
    }

    Ok(rows)
}

fn read_all_excel_rows(
    file_path: &Path,
    requested_sheet: Option<&str>,
) -> Result<Vec<SourceRow>, String> {
    let mut workbook: Sheets<_> = open_workbook_auto(file_path)
        .map_err(|e| format!("Không thể mở bảng tính Excel: {}", e))?;

    let sheet_names = workbook.sheet_names().to_vec();
    let selected_sheet = requested_sheet
        .map(String::from)
        .or_else(|| sheet_names.first().cloned())
        .ok_or_else(|| "Không tìm thấy sheet nào trong file Excel".to_string())?;

    let range = workbook
        .worksheet_range(&selected_sheet)
        .map_err(|e| format!("Lỗi đọc sheet '{}': {}", selected_sheet, e))?;

    let start_row_offset = range.start().map(|(r, _)| r as usize).unwrap_or(0);
    let mut rows = Vec::with_capacity(range.height());
    for (r_idx, row) in range.rows().enumerate() {
        let source_row_number = start_row_offset + r_idx + 1;
        let row_vec: Vec<String> = row.iter().map(cell_to_string).collect();
        rows.push(SourceRow {
            source_row_number,
            cells: row_vec,
        });
    }
    Ok(rows)
}
