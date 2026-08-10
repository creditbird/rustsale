import { describe, expect, test } from 'bun:test';
import { buildGroupMembersCsv } from '../groupMembersCsv';

describe('buildGroupMembersCsv', () => {
  test('keeps stable quoted columns, UTF-8 BOM, commas, quotes, and line breaks', () => {
    const csv = buildGroupMembersCsv([
      {
        displayName: 'Nguyễn, "An"\nVIP',
        zaloUid: '123456789',
        role: 'admin',
        crmStatus: 'New',
        categories: ['Khách, VIP', 'Miền Bắc']
      }
    ]);

    expect(csv.startsWith('\uFEFF')).toBe(true);
    expect(csv).toBe(
      '\uFEFF"DisplayName","Zalo_UID","Role","CRM_Status","Categories"\r\n' +
        '"Nguyễn, ""An""\nVIP","123456789","admin","New","Khách, VIP; Miền Bắc"'
    );
  });

  test('neutralizes formulas even after leading whitespace', () => {
    const csv = buildGroupMembersCsv([
      {
        displayName: '=SUM(1,2)',
        zaloUid: '  +441234',
        role: '\t-1',
        crmStatus: '@cmd',
        categories: [' normal', ' -danger']
      }
    ]);

    expect(csv).toContain('"\'=SUM(1,2)"');
    expect(csv).toContain('"\'  +441234"');
    expect(csv).toContain('"\'\t-1"');
    expect(csv).toContain('"\'@cmd"');
    expect(csv).toContain('" normal;  -danger"');
  });
});
