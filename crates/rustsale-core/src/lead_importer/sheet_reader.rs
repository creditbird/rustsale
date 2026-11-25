use calamine::{open_workbook_auto, Cell, Data, DataRef, Range, Reader, Sheets, XlsxError};
use std::collections::BTreeMap;
use std::path::Path;

use crate::lead_importer::importer::{MAX_IMPORT_COLS, MAX_IMPORT_ROWS};
use crate::lead_importer::inspector::cell_to_string;
use crate::lead_importer::types::{ImportInspection, SourceRow};

const INSPECT_PREVIEW_ROWS: usize = 11;

fn sheet_too_large_error(sheet_name: &str, rows: u64, cols: u64) -> String {
    format!(
        "Trang tính '{}' quá lớn: {} dòng x {} cột (giới hạn tối đa {} dòng, {} cột). Vui lòng xóa các ô thừa ở xa hoặc tách nhỏ tệp trước khi nhập.",
        sheet_name, rows, cols, MAX_IMPORT_ROWS, MAX_IMPORT_COLS
    )
}

#[derive(Default)]
struct SheetBounds {
    bounds: Option<(u32, u32, u32, u32)>,
}

impl SheetBounds {
    fn min_row(&self) -> Option<u32> {
        self.bounds.map(|(min_row, _, _, _)| min_row)
    }

    fn col_span(&self) -> usize {
        self.bounds.map_or(0, |(_, _, min_col, max_col)| {
            (max_col - min_col) as usize + 1
        })
    }

    fn as_tuple(&self) -> Option<(u32, u32, u32, u32)> {
        self.bounds
    }

    fn record_cell(
        &mut self,
        sheet_name: &str,
        row: u32,
        col: u32,
    ) -> Result<(u32, u32, u32, u32), String> {
        let updated = match self.bounds {
            Some((r0, r1, c0, c1)) => (r0.min(row), r1.max(row), c0.min(col), c1.max(col)),
            None => (row, row, col, col),
        };
        let row_span = (updated.1 as u64 - updated.0 as u64) + 1;
        let col_span = (updated.3 as u64 - updated.2 as u64) + 1;
        if row_span > MAX_IMPORT_ROWS as u64 || col_span > MAX_IMPORT_COLS as u64 {
            return Err(sheet_too_large_error(sheet_name, row_span, col_span));
        }
        self.bounds = Some(updated);
        Ok(updated)
    }
}

fn cell_ref_to_string(cell: &DataRef<'_>) -> String {
    match cell {
        DataRef::Empty => String::new(),
        DataRef::String(s) => s.trim().to_string(),
        DataRef::SharedString(s) => s.trim().to_string(),
        DataRef::Float(f) => {
            if f.fract() == 0.0 && *f >= -1e15 && *f <= 1e15 {
                format!("{:.0}", f)
            } else {
                format!("{}", f)
            }
        }
        DataRef::Int(i) => format!("{}", i),
        DataRef::Bool(b) => format!("{}", b),
        DataRef::DateTime(d) => format!("{:.2}", d),
        DataRef::DateTimeIso(s) => s.clone(),
        DataRef::DurationIso(s) => s.clone(),
        DataRef::Error(_) => String::new(),
    }
}

fn validate_range_bounds(sheet_name: &str, range: &Range<Data>) -> Result<(), String> {
    let rows = range.height() as u64;
    let cols = range.width() as u64;
    if rows > MAX_IMPORT_ROWS as u64 || cols > MAX_IMPORT_COLS as u64 {
        return Err(sheet_too_large_error(sheet_name, rows, cols));
    }
    Ok(())
}

fn inspect_from_range(
    sheet_name: &str,
    range: &Range<Data>,
) -> Result<(Vec<Vec<String>>, usize), String> {
    validate_range_bounds(sheet_name, range)?;

    let total_rows = range.height();
    let total_cols = range.width();
    if total_rows == 0 || total_cols == 0 {
        return Err(format!("Trang tính '{}' không có dữ liệu", sheet_name));
    }

    let mut rows_sample: Vec<Vec<String>> =
        Vec::with_capacity(total_rows.min(INSPECT_PREVIEW_ROWS));
    for row in range.rows().take(INSPECT_PREVIEW_ROWS) {
        let mut row_vec = Vec::with_capacity(total_cols);
        for cell in row {
            row_vec.push(cell_to_string(cell));
        }
        rows_sample.push(row_vec);
    }

    Ok((rows_sample, total_rows))
}

fn read_rows_from_range(sheet_name: &str, range: &Range<Data>) -> Result<Vec<SourceRow>, String> {
    validate_range_bounds(sheet_name, range)?;

    let start_row_offset = range.start().map_or(0, |(r, _)| r as usize);
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

fn inspect_from_cell_stream<'a, F>(
    sheet_name: &str,
    mut next_cell: F,
) -> Result<(Vec<Vec<String>>, usize), String>
where
    F: FnMut() -> Result<Option<Cell<DataRef<'a>>>, String>,
{
    let mut bounds = SheetBounds::default();
    let mut preview_rows: BTreeMap<u32, Vec<(u32, String)>> = BTreeMap::new();

    while let Some(cell) = next_cell()? {
        if matches!(cell.get_value(), DataRef::Empty) {
            continue;
        }
        let (row, col) = cell.get_position();
        let prev_min_row = bounds.min_row();
        let (cur_min_row, _, _, _) = bounds.record_cell(sheet_name, row, col)?;

        let preview_end = cur_min_row.saturating_add(INSPECT_PREVIEW_ROWS as u32);
        if prev_min_row.is_some_and(|prev| cur_min_row < prev) {
            let _ = preview_rows.split_off(&preview_end);
        }
        if row < preview_end {
            let val = cell_ref_to_string(cell.get_value());
            preview_rows.entry(row).or_default().push((col, val));
        }
    }

    let Some((min_row, max_row, min_col, max_col)) = bounds.as_tuple() else {
        return Err(format!("Trang tính '{}' không có dữ liệu", sheet_name));
    };

    let total_rows = (max_row - min_row) as usize + 1;
    let total_cols = (max_col - min_col) as usize + 1;
    let preview_count = total_rows.min(INSPECT_PREVIEW_ROWS);

    let mut rows_sample = Vec::with_capacity(preview_count);
    for offset in 0..preview_count {
        let target_row = min_row + offset as u32;
        let mut row_vec = vec![String::new(); total_cols];
        if let Some(entries) = preview_rows.remove(&target_row) {
            for (col, val) in entries {
                if let Some(slot) = col
                    .checked_sub(min_col)
                    .and_then(|c| row_vec.get_mut(c as usize))
                {
                    *slot = val;
                }
            }
        }
        rows_sample.push(row_vec);
    }

    Ok((rows_sample, total_rows))
}

fn read_rows_from_cell_stream<'a, F>(
    sheet_name: &str,
    mut next_cell: F,
) -> Result<Vec<SourceRow>, String>
where
    F: FnMut() -> Result<Option<Cell<DataRef<'a>>>, String>,
{
    let mut bounds = SheetBounds::default();
    let mut sparse_rows: Vec<(u32, Vec<(u32, String)>)> = Vec::new();

    while let Some(cell) = next_cell()? {
        if matches!(cell.get_value(), DataRef::Empty) {
            continue;
        }
        let (row, col) = cell.get_position();
        bounds.record_cell(sheet_name, row, col)?;
        let val = cell_ref_to_string(cell.get_value());

        match sparse_rows.last_mut() {
            Some((last_row, last_cells)) if *last_row == row => {
                last_cells.push((col, val));
            }
            _ => {
                let col_hint = bounds.col_span().clamp(1, 16);
                let mut cells = Vec::with_capacity(col_hint);
                cells.push((col, val));
                sparse_rows.push((row, cells));
            }
        }
    }

    let Some((min_row, max_row, min_col, _max_col)) = bounds.as_tuple() else {
        return Ok(Vec::new());
    };

    let row_span = (max_row - min_row) as usize + 1;
    let mut result: Vec<SourceRow> = (0..row_span)
        .map(|offset| SourceRow {
            source_row_number: min_row as usize + offset + 1,
            cells: Vec::new(),
        })
        .collect();

    for (row, row_entries) in sparse_rows {
        let Some(row_offset) = row.checked_sub(min_row).map(|r| r as usize) else {
            continue;
        };
        let Some(target) = result.get_mut(row_offset) else {
            continue;
        };
        let Some(row_max_col) = row_entries.iter().map(|(c, _)| *c).max() else {
            continue;
        };
        let Some(needed_len) = row_max_col.checked_sub(min_col).map(|c| c as usize + 1) else {
            continue;
        };
        if target.cells.len() < needed_len {
            target.cells.resize(needed_len, String::new());
        }
        for (col, val) in row_entries {
            if let Some(slot) = col
                .checked_sub(min_col)
                .and_then(|c| target.cells.get_mut(c as usize))
            {
                *slot = val;
            }
        }
    }

    Ok(result)
}

/// Inspects an Excel/ODS workbook without constructing an unbounded dense `Range`.
///
/// For `.xlsx` and `.xlsb`, cells are streamed via calamine's cell reader so sheets
/// with stray far-out cells (e.g. `XFD1048576`) are rejected before allocating.
/// For `.xls` and `.ods`, calamine 0.38 eagerly parses sheets into `Range<Data>` during
/// `open_workbook_auto` (no streaming API is exposed); we still enforce
/// `MAX_IMPORT_ROWS` / `MAX_IMPORT_COLS` on the resulting range before materializing rows.
pub(crate) fn inspect_excel_sheet(
    file_path: &Path,
    requested_sheet: Option<&str>,
) -> Result<ImportInspection, String> {
    let mut workbook: Sheets<_> = open_workbook_auto(file_path)
        .map_err(|e| format!("Không thể mở bảng tính Excel: {}", e))?;

    let sheet_names = workbook.sheet_names().to_vec();
    if sheet_names.is_empty() {
        return Err("Tệp Excel không chứa trang tính (sheet) nào".to_string());
    }

    let selected_sheet = requested_sheet
        .map(String::from)
        .or_else(|| sheet_names.first().cloned())
        .ok_or_else(|| "Không tìm thấy sheet hợp lệ".to_string())?;

    let (rows_sample, total_rows) = match &mut workbook {
        Sheets::Xlsx(xlsx) => match xlsx.worksheet_cells_reader(&selected_sheet) {
            Ok(mut reader) => inspect_from_cell_stream(&selected_sheet, || {
                reader
                    .next_cell()
                    .map_err(|e| format!("Lỗi đọc sheet '{}': {}", selected_sheet, e))
            })?,
            Err(XlsxError::NotAWorksheet(_)) => {
                return Err(format!("Trang tính '{}' không có dữ liệu", selected_sheet));
            }
            Err(e) => return Err(format!("Lỗi đọc sheet '{}': {}", selected_sheet, e)),
        },
        Sheets::Xlsb(xlsb) => {
            let mut reader = xlsb
                .worksheet_cells_reader(&selected_sheet)
                .map_err(|e| format!("Lỗi đọc sheet '{}': {}", selected_sheet, e))?;
            inspect_from_cell_stream(&selected_sheet, || {
                reader
                    .next_cell()
                    .map_err(|e| format!("Lỗi đọc sheet '{}': {}", selected_sheet, e))
            })?
        }
        Sheets::Xls(xls) => {
            let range = xls
                .worksheet_range(&selected_sheet)
                .map_err(|e| format!("Lỗi đọc sheet '{}': {}", selected_sheet, e))?;
            inspect_from_range(&selected_sheet, &range)?
        }
        Sheets::Ods(ods) => {
            let range = ods
                .worksheet_range(&selected_sheet)
                .map_err(|e| format!("Lỗi đọc sheet '{}': {}", selected_sheet, e))?;
            inspect_from_range(&selected_sheet, &range)?
        }
    };

    let headers = rows_sample.first().cloned().unwrap_or_default();
    let sample_rows = if rows_sample.len() > 1 {
        rows_sample[1..].to_vec()
    } else {
        Vec::new()
    };

    Ok(ImportInspection {
        file_type: "excel".to_string(),
        sheets: sheet_names,
        selected_sheet: Some(selected_sheet),
        headers,
        sample_rows,
        total_rows_estimate: total_rows.saturating_sub(1),
    })
}

/// Reads all rows of an Excel/ODS sheet for lead import with bounded sparse loading.
///
/// Same format coverage and calamine 0.38 `.xls`/`.ods` caveat as [`inspect_excel_sheet`].
pub(crate) fn read_excel_sheet_rows(
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

    match &mut workbook {
        Sheets::Xlsx(xlsx) => match xlsx.worksheet_cells_reader(&selected_sheet) {
            Ok(mut reader) => read_rows_from_cell_stream(&selected_sheet, || {
                reader
                    .next_cell()
                    .map_err(|e| format!("Lỗi đọc sheet '{}': {}", selected_sheet, e))
            }),
            Err(XlsxError::NotAWorksheet(_)) => Ok(Vec::new()),
            Err(e) => Err(format!("Lỗi đọc sheet '{}': {}", selected_sheet, e)),
        },
        Sheets::Xlsb(xlsb) => {
            let mut reader = xlsb
                .worksheet_cells_reader(&selected_sheet)
                .map_err(|e| format!("Lỗi đọc sheet '{}': {}", selected_sheet, e))?;
            read_rows_from_cell_stream(&selected_sheet, || {
                reader
                    .next_cell()
                    .map_err(|e| format!("Lỗi đọc sheet '{}': {}", selected_sheet, e))
            })
        }
        Sheets::Xls(xls) => {
            let range = xls
                .worksheet_range(&selected_sheet)
                .map_err(|e| format!("Lỗi đọc sheet '{}': {}", selected_sheet, e))?;
            read_rows_from_range(&selected_sheet, &range)
        }
        Sheets::Ods(ods) => {
            let range = ods
                .worksheet_range(&selected_sheet)
                .map_err(|e| format!("Lỗi đọc sheet '{}': {}", selected_sheet, e))?;
            read_rows_from_range(&selected_sheet, &range)
        }
    }
}
