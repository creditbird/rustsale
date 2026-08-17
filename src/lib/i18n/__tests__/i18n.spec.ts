import { describe, it, expect, beforeEach } from 'bun:test';
import { m, setLocale, getLocale } from '../index';
import {
  formatCurrency,
  formatCompactCurrency,
  formatDate,
  formatDateTime,
  formatNumber,
  formatRelativeTime
} from '../formatters';

describe('i18n & Paraglide Integration', () => {
  beforeEach(() => {
    setLocale('vi');
  });

  it('correctly translates common keys into Vietnamese when locale is vi', () => {
    setLocale('vi');
    expect(getLocale()).toBe('vi');
    expect(String(m.app_name())).toBe('RustSale CRM');
    expect(String(m.common_save())).toBe('Lưu');
    expect(String(m.common_cancel())).toBe('Hủy');
    expect(String(m.common_clear_filters())).toBe('Xóa bộ lọc');
    expect(String(m.nav_dashboard())).toBe('Bảng điều khiển');
    expect(String(m.nav_leads())).toBe('Khách hàng tiềm năng');
    expect(String(m.pipeline_stage_prospecting())).toBe('Tìm kiếm Tiềm năng');
    expect(String(m.pipeline_stage_won())).toBe('Thành công');
  });

  it('correctly translates common keys into English when locale is en', () => {
    setLocale('en');
    expect(getLocale()).toBe('en');
    expect(String(m.common_save())).toBe('Save');
    expect(String(m.common_cancel())).toBe('Cancel');
    expect(String(m.common_clear_filters())).toBe('Clear filters');
    expect(String(m.nav_dashboard())).toBe('Dashboard');
    expect(String(m.nav_leads())).toBe('Leads');
    expect(String(m.pipeline_stage_prospecting())).toBe('Prospecting');
    expect(String(m.pipeline_stage_won())).toBe('Closed Won');
  });

  it('correctly interpolates parameters in messages', () => {
    setLocale('vi');
    expect(String(m.common_total({ count: 42 }))).toBe('Tổng: 42');
    expect(String(m.scraper_results_found({ count: 15 }))).toBe('Đã tìm thấy 15 địa điểm');

    setLocale('en');
    expect(String(m.common_total({ count: 42 }))).toBe('Total: 42');
    expect(String(m.scraper_results_found({ count: 15 }))).toBe('15 places found');
  });

  it('does NOT infer business currency from UI locale (financial data integrity)', () => {
    // Default base currency is VND regardless of UI locale
    setLocale('vi');
    const viVnd = formatCurrency(5000000);
    expect(viVnd).toContain('5.000.000');
    expect(viVnd).toContain('₫');

    setLocale('en');
    const enVnd = formatCurrency(5000000);
    // In English locale, 5,000,000 VND must still format as VND, NEVER inferred as USD ($)
    expect(enVnd).toContain('5,000,000');
    expect(enVnd).toContain('₫');
    expect(enVnd).not.toContain('$');

    // Explicit USD format
    const enUsd = formatCurrency(1500.5, 'USD');
    expect(enUsd).toContain('1,500.50');
    expect(enUsd).toContain('$');

    setLocale('vi');
    const viUsd = formatCurrency(1500.5, 'USD');
    expect(viUsd).toContain('1.500,50');
  });

  it('correctly formats compact currency notation for VND and USD', () => {
    setLocale('vi');
    expect(formatCompactCurrency(1500000000, 'VND')).toBe('1,5 Tỷ ₫');
    expect(formatCompactCurrency(25000000, 'VND')).toBe('25 Tr ₫');
    expect(formatCompactCurrency(50000, 'VND')).toBe('50k ₫');
    expect(formatCompactCurrency(500, 'VND')).toContain('500');

    setLocale('en');
    expect(formatCompactCurrency(1500000000, 'VND')).toBe('1.5B ₫');
    expect(formatCompactCurrency(25000000, 'VND')).toBe('25M ₫');
    expect(formatCompactCurrency(50000, 'VND')).toBe('50k ₫');
  });

  it('formats numbers according to active locale', () => {
    setLocale('vi');
    const viNum = formatNumber(1234567.89);
    expect(viNum).toBe('1.234.567,89');

    setLocale('en');
    const enNum = formatNumber(1234567.89);
    expect(enNum).toBe('1,234,567.89');
  });

  it('formats dates properly including timestamp 0', () => {
    const testDate = new Date('2026-08-17T12:00:00Z');
    setLocale('vi');
    const viDate = formatDate(testDate);
    expect(viDate).toContain('2026');

    setLocale('en');
    const enDate = formatDate(testDate);
    expect(enDate).toContain('2026');

    // Epoch timestamp 0
    const epochDate = formatDate(0);
    expect(epochDate).toContain('1970');
  });

  it('handles invalid or empty inputs gracefully in formatters', () => {
    expect(formatCurrency(null)).toBe('0 ₫');
    expect(formatCurrency(null, 'USD')).toBe('0 $');
    expect(formatNumber(null)).toBe('0');
    expect(formatDate(null)).toBe('');
    expect(formatDate('')).toBe('');
    expect(formatDateTime(null)).toBe('');
    expect(formatDateTime('')).toBe('');
    expect(formatRelativeTime(null)).toBe('');
    expect(formatRelativeTime('')).toBe('');
  });

  it('guarantees 100% key parity and non-empty values between en.json and vi.json', async () => {
    const enFile = await Bun.file('messages/en.json').json();
    const viFile = await Bun.file('messages/vi.json').json();

    const enKeys = Object.keys(enFile).sort();
    const viKeys = Object.keys(viFile).sort();

    const missingInVi = enKeys.filter((k) => !(k in viFile));
    const missingInEn = viKeys.filter((k) => !(k in enFile));

    expect(missingInVi).toEqual([]);
    expect(missingInEn).toEqual([]);
    expect(enKeys.length).toBe(viKeys.length);

    // Verify all keys have non-empty string content
    for (const key of enKeys) {
      if (key === '$schema') continue;
      expect(typeof enFile[key]).toBe('string');
      expect(enFile[key].trim().length).toBeGreaterThan(0);
      expect(typeof viFile[key]).toBe('string');
      expect(viFile[key].trim().length).toBeGreaterThan(0);
    }
  });
});
