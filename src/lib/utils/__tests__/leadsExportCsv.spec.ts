/// <reference types="bun-types" />
import { describe, expect, test } from 'bun:test';
import type { Lead } from '$lib/services/crmService';
import { buildLeadsCsv, exportLeadsToCsv } from '../leadsExportCsv';

describe('leadsExportCsv Utility', () => {
  const mockLeads: Lead[] = [
    {
      id: 1,
      name: 'Đau Cổ Vai Gáy',
      company: 'Công ty TNHH HUKI',
      status: 'New',
      value: 50000,
      last_activity: '2026-08-14',
      owner: 'Nguyễn Văn A',
      email: 'huki@example.com',
      phone: '0353300088',
      address: '123 Đường Số 1, TP.HCM',
      about: 'Khách hàng quan tâm gói VIP\nCó hẹn gọi lại',
      has_zalo: 1,
      zalo_status: 'verified',
      zalo_uid: 'zalo_12345'
    },
    {
      id: 2,
      name: '=SUM(1,2)',
      company: 'Tech "Plus" Corp',
      status: 'Converted',
      value: 0,
      last_activity: '2026-08-10',
      owner: 'Admin',
      email: 'tech@corp.com',
      phone: '0909410040',
      address: null,
      about: null,
      has_zalo: 0,
      zalo_status: null,
      zalo_uid: null
    },
    {
      id: 3,
      name: 'Khách hàng chưa kiểm tra',
      company: 'Công ty ABC',
      status: 'Contacted',
      value: 12000,
      last_activity: '2026-08-01',
      owner: 'Self',
      email: '',
      phone: '+84981234567',
      address: 'Hà Nội',
      about: '',
      has_zalo: null
    }
  ];

  test('generates valid CSV with UTF-8 BOM, headers, and properly formatted fields', () => {
    const csv = buildLeadsCsv(mockLeads);

    expect(csv.startsWith('\uFEFF')).toBe(true);

    const lines = csv.replace('\uFEFF', '').split('\r\n');
    expect(lines.length).toBeGreaterThanOrEqual(4); // Header + 3 data rows

    const header = lines[0];
    expect(header).toContain('"Name"');
    expect(header).toContain('"Company"');
    expect(header).toContain('"Phone"');
    expect(header).toContain('"Email"');
    expect(header).toContain('"Status"');
    expect(header).toContain('"Value"');
    expect(header).toContain('"Zalo_Status"');
    expect(header).toContain('"Zalo_UID"');
    expect(header).toContain('"Owner"');
    expect(header).toContain('"Address"');
    expect(header).toContain('"About"');
    expect(header).toContain('"Last_Activity"');
  });

  test('preserves leading zero for Vietnamese phone numbers and escapes quotes/multiline', () => {
    const csv = buildLeadsCsv(mockLeads);

    // Vietnamese phone number 0353300088 should be preserved with text prefix '0353300088
    expect(csv).toContain('"\'0353300088"');
    expect(csv).toContain('"\'0909410040"');

    // Quotes should be doubled in CSV standard: Tech "Plus" Corp -> Tech ""Plus"" Corp
    expect(csv).toContain('"Tech ""Plus"" Corp"');

    // Multiline about should be preserved inside quotes
    expect(csv).toContain('Khách hàng quan tâm gói VIP\nCó hẹn gọi lại');
  });

  test('neutralizes formulas to prevent CSV injection', () => {
    const csv = buildLeadsCsv(mockLeads);

    // =SUM(1,2) should be neutralized to '=SUM(1,2)
    expect(csv).toContain('"\'=SUM(1,2)"');
  });

  test('correctly maps Zalo status labels (Verified, No Zalo, Unchecked)', () => {
    const csv = buildLeadsCsv(mockLeads);

    expect(csv).toContain('"Verified"');
    expect(csv).toContain('"No Zalo"');
    expect(csv).toContain('"Unchecked"');
  });

  test('handles empty lead array without error', () => {
    const csv = buildLeadsCsv([]);
    expect(csv.startsWith('\uFEFF')).toBe(true);
    const lines = csv.replace('\uFEFF', '').split('\r\n').filter(Boolean);
    expect(lines.length).toBe(1); // Header only
  });
});
