import { invoke, isTauri } from '@tauri-apps/api/core';
import { Channel } from '@tauri-apps/api/core';
import { sanitizeCsvField } from '$lib/utils/leadsExportCsv';

export interface ImportInspection {
  file_type: 'excel' | 'csv';
  sheets: string[];
  selected_sheet?: string | null;
  headers: string[];
  sample_rows: string[][];
  total_rows_estimate: number;
}

export interface ColumnMapping {
  name_col?: number | null;
  company_col?: number | null;
  phone_col?: number | null;
  email_col?: number | null;
  status_col?: number | null;
  value_col?: number | null;
  address_col?: number | null;
  about_col?: number | null;
  owner_col?: number | null;
}

export type ConflictStrategy = 'skip' | 'overwrite' | 'allow';

export interface LeadImportOptions {
  file_path: string;
  sheet_name?: string | null;
  has_header: boolean;
  header_row_index: number;
  mapping: ColumnMapping;
  conflict_strategy: ConflictStrategy;
  default_category_id?: number | null;
  default_status?: string | null;
  default_owner?: string | null;
  default_currency?: string | null;
  normalize_phone: boolean;
}

export interface ImportProgress {
  processed_rows: number;
  total_rows: number;
  percentage: number;
}

export interface ImportErrorRow {
  row_number: number;
  raw_data: string[];
  reason: string;
}

export interface ImportSummaryResult {
  total_rows: number;
  inserted: number;
  updated: number;
  skipped_duplicates: number;
  failed: number;
  error_rows: ImportErrorRow[];
}

export function isTauriEnvironment(): boolean {
  if (typeof window === 'undefined') return false;
  return Boolean(
    (window as any).__TAURI_INTERNALS__ ||
    (window as any).__TAURI__ ||
    (typeof isTauri === 'function' && isTauri())
  );
}

/**
 * Automatically inspects column header names to detect suitable mappings for standard CRM fields.
 */
export function autoDetectColumnMapping(headers: string[]): ColumnMapping {
  const mapping: ColumnMapping = {
    name_col: null,
    company_col: null,
    phone_col: null,
    email_col: null,
    status_col: null,
    value_col: null,
    address_col: null,
    about_col: null,
    owner_col: null
  };

  const clean = (h: string) => h.toLowerCase().trim().replace(/[(*)_]/g, '');

  headers.forEach((h, idx) => {
    const text = clean(h);

    if (mapping.name_col === null && /(họ\s*và\s*tên|họ\s*tên|^tên|full\s*name|^name|khách\s*hàng)/i.test(text)) {
      mapping.name_col = idx;
    } else if (mapping.phone_col === null && /(điện\s*thoại|sđt|phone|mobile|tel|liên\s*hệ)/i.test(text)) {
      mapping.phone_col = idx;
    } else if (mapping.email_col === null && /(email|thư\s*điện\s*tử|e-mail|mail)/i.test(text)) {
      mapping.email_col = idx;
    } else if (mapping.company_col === null && /(công\s*ty|doanh\s*nghiệp|đơn\s*vị|company|corp|organization)/i.test(text)) {
      mapping.company_col = idx;
    } else if (mapping.value_col === null && /(giá\s*trị|ngân\s*sách|tiền|value|budget|deal|amount)/i.test(text)) {
      mapping.value_col = idx;
    } else if (mapping.address_col === null && /(địa\s*chỉ|address|nơi\s*ở|location|khu\s*vực)/i.test(text)) {
      mapping.address_col = idx;
    } else if (mapping.about_col === null && /(ghi\s*chú|mô\s*tả|note|about|description|yêu\s*cầu)/i.test(text)) {
      mapping.about_col = idx;
    } else if (mapping.status_col === null && /(trạng\s*thái|status|tình\s*trạng|stage)/i.test(text)) {
      mapping.status_col = idx;
    } else if (mapping.owner_col === null && /(phụ\s*trách|owner|nhân\s*viên|assignee)/i.test(text)) {
      mapping.owner_col = idx;
    }
  });

  return mapping;
}

/**
 * Builds a downloadable UTF-8 BOM CSV containing failed / quarantined rows along with the error reason.
 */
export function buildErrorRowsCsv(originalHeaders: string[], errorRows: ImportErrorRow[]): string {
  const headers = [...originalHeaders, 'Lý do từ chối (Rejection Reason)']
    .map((h) => sanitizeCsvField(h))
    .join(',');

  const rows = errorRows.map((err) => {
    const padded = [...err.raw_data];
    while (padded.length < originalHeaders.length) {
      padded.push('');
    }

    const cells = padded.slice(0, originalHeaders.length).map((c) => sanitizeCsvField(c));
    cells.push(sanitizeCsvField(err.reason));
    return cells.join(',');
  });

  return `\uFEFF${[headers, ...rows].join('\r\n')}`;
}

export async function pickLeadImportFile(): Promise<string | null> {
  if (!isTauriEnvironment()) {
    throw new Error('Hộp thoại chọn tệp chỉ khả dụng trên ứng dụng máy tính (Tauri).');
  }
  return invoke<string | null>('pick_lead_import_file');
}

export async function inspectLeadImportFile(
  filePath: string,
  sheetName?: string | null
): Promise<ImportInspection> {
  if (!isTauriEnvironment()) {
    throw new Error('Tính năng kiểm tra tệp chỉ khả dụng trên ứng dụng máy tính (Tauri).');
  }
  return invoke<ImportInspection>('inspect_lead_import_file', {
    filePath,
    sheetName: sheetName ?? null
  });
}

export async function executeLeadImport(
  options: LeadImportOptions,
  onProgress?: (progress: ImportProgress) => void
): Promise<ImportSummaryResult> {
  if (!isTauriEnvironment()) {
    throw new Error('Tính năng nhập dữ liệu chỉ khả dụng trên ứng dụng máy tính (Tauri).');
  }

  const channel = new Channel<ImportProgress>();
  if (onProgress) {
    channel.onmessage = (progress) => {
      onProgress(progress);
    };
  }

  return invoke<ImportSummaryResult>('execute_lead_import', {
    options,
    onProgress: channel
  });
}

export async function downloadLeadImportTemplate(format: 'xlsx' | 'csv'): Promise<string | null> {
  if (!isTauriEnvironment()) {
    throw new Error('Tính năng lưu file mẫu chỉ khả dụng trên ứng dụng máy tính (Tauri).');
  }
  return invoke<string | null>('download_lead_import_template', { format });
}
