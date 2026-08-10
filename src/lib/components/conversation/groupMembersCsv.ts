export interface GroupMemberCsvRow {
  displayName: string;
  zaloUid: string;
  role: string;
  crmStatus: string;
  categories: string[];
}

const CSV_HEADERS = ['DisplayName', 'Zalo_UID', 'Role', 'CRM_Status', 'Categories'] as const;
const SPREADSHEET_FORMULA_PREFIX = /^\s*[=+\-@]/u;

function quoteCsvValue(value: string): string {
  const neutralized = SPREADSHEET_FORMULA_PREFIX.test(value) ? `'${value}` : value;
  return `"${neutralized.replace(/"/g, '""')}"`;
}

export function buildGroupMembersCsv(rows: readonly GroupMemberCsvRow[]): string {
  const lines = [
    CSV_HEADERS.map(quoteCsvValue).join(','),
    ...rows.map((row) =>
      [
        row.displayName,
        row.zaloUid,
        row.role,
        row.crmStatus,
        row.categories.join('; ')
      ]
        .map(quoteCsvValue)
        .join(',')
    )
  ];

  return `\uFEFF${lines.join('\r\n')}`;
}
