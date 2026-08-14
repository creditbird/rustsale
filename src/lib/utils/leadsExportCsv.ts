import type { Lead } from '$lib/services/crmService';

export const CSV_LEAD_HEADERS = [
  'Name',
  'Company',
  'Phone',
  'Email',
  'Status',
  'Value',
  'Zalo_Status',
  'Zalo_UID',
  'Owner',
  'Address',
  'About',
  'Last_Activity'
] as const;

const SPREADSHEET_FORMULA_PREFIX = /^\s*[=+\-@\t\r]/u;
const VIETNAMESE_PHONE_PREFIX = /^\s*0\d+/u;

/**
 * Escapes and neutralizes a value for safe CSV output.
 * - Adds leading `'` if the value starts with spreadsheet formula symbols (=, +, -, @).
 * - Adds leading `'` if the value starts with `0` (e.g. Vietnamese phone number) so Excel preserves the leading zero.
 * - Doubles any double quotes and wraps the string in double quotes.
 */
export function sanitizeCsvField(value: string | number | null | undefined): string {
  if (value === null || value === undefined) {
    return '""';
  }
  const str = String(value);
  
  let neutralized = str;
  if (SPREADSHEET_FORMULA_PREFIX.test(str) || VIETNAMESE_PHONE_PREFIX.test(str)) {
    neutralized = `'${str}`;
  }
  
  return `"${neutralized.replace(/"/g, '""')}"`;
}

/**
 * Maps raw has_zalo & zalo_status values into human-readable labels.
 */
export function formatZaloStatus(lead: Lead): string {
  if (lead.has_zalo === 1) return 'Verified';
  if (lead.has_zalo === 0) return 'No Zalo';
  return 'Unchecked';
}

/**
 * Converts a list of Lead records into a UTF-8 BOM CSV formatted string.
 */
export function buildLeadsCsv(leads: readonly Lead[]): string {
  const headerRow = CSV_LEAD_HEADERS.map((h) => `"${h}"`).join(',');

  const dataRows = leads.map((lead) => {
    return [
      sanitizeCsvField(lead.name || ''),
      sanitizeCsvField(lead.company || ''),
      sanitizeCsvField(lead.phone || ''),
      sanitizeCsvField(lead.email || ''),
      sanitizeCsvField(lead.status || ''),
      sanitizeCsvField(lead.value ?? 0),
      sanitizeCsvField(formatZaloStatus(lead)),
      sanitizeCsvField(lead.zalo_uid || ''),
      sanitizeCsvField(lead.owner || ''),
      sanitizeCsvField(lead.address || ''),
      sanitizeCsvField(lead.about || ''),
      sanitizeCsvField(lead.last_activity || '')
    ].join(',');
  });

  const lines = [headerRow, ...dataRows];
  return `\uFEFF${lines.join('\r\n')}`;
}

/**
 * Generates a timestamped filename for exported Leads or Contacts.
 */
export function generateExportFilename(entityType: 'leads' | 'contacts' = 'leads'): string {
  const date = new Date();
  const yyyy = date.getFullYear();
  const mm = String(date.getMonth() + 1).padStart(2, '0');
  const dd = String(date.getDate()).padStart(2, '0');
  const hh = String(date.getHours()).padStart(2, '0');
  const min = String(date.getMinutes()).padStart(2, '0');
  const ss = String(date.getSeconds()).padStart(2, '0');
  const prefix = entityType === 'contacts' ? 'RustSale_Contacts' : 'RustSale_Leads';
  return `${prefix}_${yyyy}${mm}${dd}_${hh}${min}${ss}.csv`;
}

import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';

/**
 * Triggers a native Save File Dialog (or browser fallback) to save the CSV file.
 * Returns the saved path/filename, or null if the user cancelled the dialog.
 */
export async function exportLeadsToCsv(
  leads: readonly Lead[],
  entityType: 'leads' | 'contacts' = 'leads'
): Promise<string | null> {
  const csvContent = buildLeadsCsv(leads);
  const defaultFilename = generateExportFilename(entityType);

  try {
    const selectedPath = await save({
      defaultPath: defaultFilename,
      filters: [{ name: 'CSV File (*.csv)', extensions: ['csv'] }]
    });

    if (!selectedPath) {
      return null;
    }

    await invoke('write_text_file', { path: selectedPath, content: csvContent });
    return selectedPath;
  } catch {
    if (typeof document !== 'undefined') {
      const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' });
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = url;
      link.download = defaultFilename;
      link.click();
      URL.revokeObjectURL(url);
      return defaultFilename;
    }
    return defaultFilename;
  }
}

