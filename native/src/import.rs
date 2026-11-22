//! Lead import and export: file dialog, inspection, execution and templates.

use crate::error::CoreApiError;
use crate::runtime::{body, checked_optional_id32, on_core, state_of};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::commands::{self, DbState};
use rustsale_core::host::Channel;
use rustsale_core::lead_importer::types::{ImportProgress, LeadImportOptions};

/// Lead import and export. The import runs on a worker and reports progress through the
/// `lead-import-progress` event on `Host.onEvent`; the file dialogs are served by the host's
/// `pick_paths`/`pick_save_path`, so they only work in a desktop app.
#[module]
pub struct ImportLeads;

#[module]
impl ImportLeads {
    /// Opens the picker for an Excel or CSV file and resolves its path, or `null` when cancelled.
    pub async fn pick_file() -> Result<JsonValue, JsError> {
        on_core(move |h| async move {
            Ok::<_, CoreApiError>(commands::pick_lead_import_file(h).await?)
        })
        .await
    }

    /// Reads the sheets, headers and sample rows of `filePath` (optionally sheet `sheetName`) and
    /// resolves `{ file_type, sheets, selected_sheet, headers, sample_rows, total_rows_estimate }`.
    pub async fn inspect_file(
        file_path: String,
        sheet_name: Option<String>,
    ) -> Result<JsonValue, JsError> {
        on_core(move |_h| async move {
            Ok::<_, CoreApiError>(commands::inspect_lead_import_file(file_path, sheet_name).await?)
        })
        .await
    }

    /// Imports leads from `options` (the core's snake_case `LeadImportOptions`: `file_path`,
    /// `sheet_name`, `has_header`, `header_row_index`, `mapping`, `conflict_strategy`,
    /// `default_category_id`, `default_status`, `default_owner`, `default_currency`,
    /// `normalize_phone`) and resolves the `ImportSummaryResult`.
    pub async fn execute(options: JsonValue) -> Result<JsonValue, JsError> {
        let mut options: LeadImportOptions = body(options)?;
        options.default_category_id = checked_optional_id32(options.default_category_id)?;
        on_core(move |h| async move {
            let app = h.clone();
            let state = state_of::<DbState>(&h)?;
            let progress = Channel::<ImportProgress>::noop();
            Ok::<_, CoreApiError>(
                commands::execute_lead_import(app, options, progress, state).await?,
            )
        })
        .await
    }

    /// Saves the lead import template for `format` (`xlsx`/`excel` or `csv`) through the save
    /// dialog and resolves the chosen path, or `null` when cancelled.
    pub async fn download_template(format: String) -> Result<JsonValue, JsError> {
        on_core(move |h| async move {
            Ok::<_, CoreApiError>(commands::download_lead_import_template(h, format).await?)
        })
        .await
    }

    /// Saves `content` as a CSV through the save dialog using `defaultFilename` and resolves the
    /// chosen path, or `null` when cancelled.
    pub async fn export_csv(
        default_filename: String,
        content: String,
    ) -> Result<JsonValue, JsError> {
        on_core(move |h| async move {
            Ok::<_, CoreApiError>(
                commands::export_leads_csv_file(h, default_filename, content).await?,
            )
        })
        .await
    }
}
