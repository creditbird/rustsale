/// <reference types="bun-types" />
import { describe, expect, mock, test } from 'bun:test';
import type { Lead } from '$lib/services/crmService';
import {
  buildLeadsCsv,
  exportLeadsToCsv,
  formatZaloStatus,
  generateExportFilename
} from '../leadsExportCsv';

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
      zalo_status: 'VERIFIED_EXIST',
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
      zalo_status: 'RATE_LIMITED',
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
      has_zalo: null,
      zalo_status: null
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

  test('preserves explicit raw zalo_status and falls back correctly when not set', () => {
    // 1. Raw status preserved
    expect(formatZaloStatus({ name: '', company: '', status: '', value: 0, last_activity: '', owner: '', email: '', phone: '', zalo_status: 'RATE_LIMITED' })).toBe('RATE_LIMITED');
    expect(formatZaloStatus({ name: '', company: '', status: '', value: 0, last_activity: '', owner: '', email: '', phone: '', zalo_status: 'BLOCKED_STRANGER' })).toBe('BLOCKED_STRANGER');
    expect(formatZaloStatus({ name: '', company: '', status: '', value: 0, last_activity: '', owner: '', email: '', phone: '', zalo_status: 'LOOKUP_FAILED' })).toBe('LOOKUP_FAILED');
    expect(formatZaloStatus({ name: '', company: '', status: '', value: 0, last_activity: '', owner: '', email: '', phone: '', zalo_status: 'VERIFIED_EXIST' })).toBe('VERIFIED_EXIST');

    // 2. Fallbacks when zalo_status is null/empty
    expect(formatZaloStatus({ name: '', company: '', status: '', value: 0, last_activity: '', owner: '', email: '', phone: '', has_zalo: 1, zalo_status: null })).toBe('Verified');
    expect(formatZaloStatus({ name: '', company: '', status: '', value: 0, last_activity: '', owner: '', email: '', phone: '', has_zalo: 0, zalo_status: '' })).toBe('No Zalo');
    expect(formatZaloStatus({ name: '', company: '', status: '', value: 0, last_activity: '', owner: '', email: '', phone: '', has_zalo: null, zalo_status: null })).toBe('Unchecked');
  });

  test('handles empty lead array without error', () => {
    const csv = buildLeadsCsv([]);
    expect(csv.startsWith('\uFEFF')).toBe(true);
    const lines = csv.replace('\uFEFF', '').split('\r\n').filter(Boolean);
    expect(lines.length).toBe(1); // Header only
  });

  test('generateExportFilename generates valid timestamps', () => {
    const leadsFile = generateExportFilename('leads');
    expect(leadsFile).toMatch(/^RustSale_Leads_\d{8}_\d{6}\.csv$/);

    const contactsFile = generateExportFilename('contacts');
    expect(contactsFile).toMatch(/^RustSale_Contacts_\d{8}_\d{6}\.csv$/);
  });
});

describe('exportLeadsToCsv Native & Fallback Execution', () => {
  const testLeads: Lead[] = [
    {
      id: 1,
      name: 'Test Lead',
      company: 'Test Co',
      status: 'New',
      value: 1000,
      last_activity: '2026-08-14',
      owner: 'Self',
      email: 'test@example.com',
      phone: '0901234567'
    }
  ];

  test('returns null when user cancels native save dialog', async () => {
    const saveDialog = mock(() => Promise.resolve(null));
    const invokeCommand = mock((_cmd: string, _args: any) => Promise.resolve());

    const result = await exportLeadsToCsv(testLeads, 'leads', {
      saveDialog: saveDialog as any,
      invokeCommand: invokeCommand as any
    });

    expect(result).toBeNull();
    expect(invokeCommand).not.toHaveBeenCalled();
  });

  test('saves via Tauri command export_leads_csv_file when path is selected', async () => {
    const saveDialog = mock(() => Promise.resolve('/home/user/Desktop/leads_export.csv'));
    const invokeCommand = mock((_cmd: string, _args: any) => Promise.resolve());

    const result = await exportLeadsToCsv(testLeads, 'leads', {
      saveDialog: saveDialog as any,
      invokeCommand: invokeCommand as any
    });

    expect(result).toBe('/home/user/Desktop/leads_export.csv');
    expect(invokeCommand).toHaveBeenCalledWith('export_leads_csv_file', {
      path: '/home/user/Desktop/leads_export.csv',
      content: expect.stringContaining('Test Lead')
    });
  });

  test('throws error when native write command fails (no false success)', async () => {
    const saveDialog = mock(() => Promise.resolve('/restricted/leads.csv'));
    const invokeCommand = mock((_cmd: string, _args: any) => Promise.reject(new Error('Permission denied')));

    await expect(
      exportLeadsToCsv(testLeads, 'leads', {
        saveDialog: saveDialog as any,
        invokeCommand: invokeCommand as any
      })
    ).rejects.toThrow('Permission denied');
  });

  test('falls back to browser download when Tauri save dialog is unavailable', async () => {
    const saveDialog = mock(() => Promise.reject(new Error('Tauri API unavailable in browser')));

    const result = await exportLeadsToCsv(testLeads, 'leads', {
      saveDialog: saveDialog as any
    });

    expect(result).toMatch(/^RustSale_Leads_\d{8}_\d{6}\.csv$/);
  });
});
