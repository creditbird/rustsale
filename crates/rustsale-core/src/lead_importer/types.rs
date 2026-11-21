use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConflictStrategy {
    #[default]
    Skip,
    Overwrite,
    Allow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportInspection {
    pub file_type: String, // "excel" | "csv"
    pub sheets: Vec<String>,
    pub selected_sheet: Option<String>,
    pub headers: Vec<String>,
    pub sample_rows: Vec<Vec<String>>,
    pub total_rows_estimate: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ColumnMapping {
    pub name_col: Option<usize>,
    pub company_col: Option<usize>,
    pub phone_col: Option<usize>,
    pub email_col: Option<usize>,
    pub status_col: Option<usize>,
    pub value_col: Option<usize>,
    pub address_col: Option<usize>,
    pub about_col: Option<usize>,
    pub owner_col: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeadImportOptions {
    pub file_path: String,
    pub sheet_name: Option<String>,
    pub has_header: bool,
    #[serde(default)]
    pub header_row_index: usize,
    pub mapping: ColumnMapping,
    #[serde(default)]
    pub conflict_strategy: ConflictStrategy,
    pub default_category_id: Option<i32>,
    pub default_status: Option<String>,
    pub default_owner: Option<String>,
    pub default_currency: Option<String>,
    #[serde(default = "default_true")]
    pub normalize_phone: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportProgress {
    pub processed_rows: usize,
    pub total_rows: usize,
    pub percentage: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportErrorRow {
    pub row_number: usize,
    pub raw_data: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRow {
    pub source_row_number: usize,
    pub cells: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportSummaryResult {
    pub total_rows: usize,
    pub inserted: usize,
    pub updated: usize,
    pub skipped_duplicates: usize,
    pub failed: usize,
    pub error_rows: Vec<ImportErrorRow>,
}
