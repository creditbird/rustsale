import { describe, it, expect, beforeEach } from 'bun:test';
import { m, setLocale, getLocale } from '../index';
import { formatCurrency, formatDate, formatDateTime, formatNumber, formatRelativeTime } from '../formatters';

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
    expect(String(m.nav_dashboard())).toBe('Bảng điều khiển');
    expect(String(m.nav_leads())).toBe('Quản lý Leads');
  });

  it('correctly translates common keys into English when locale is en', () => {
    setLocale('en');
    expect(getLocale()).toBe('en');
    expect(String(m.common_save())).toBe('Save');
    expect(String(m.common_cancel())).toBe('Cancel');
    expect(String(m.nav_dashboard())).toBe('Dashboard');
    expect(String(m.nav_leads())).toBe('Leads');
  });

  it('correctly interpolates parameters in messages', () => {
    setLocale('vi');
    expect(String(m.common_total({ count: 42 }))).toBe('Tổng: 42');
    expect(String(m.scraper_results_found({ count: 15 }))).toBe('Tìm thấy 15 địa điểm');

    setLocale('en');
    expect(String(m.common_total({ count: 42 }))).toBe('Total: 42');
    expect(String(m.scraper_results_found({ count: 15 }))).toBe('15 places found');
  });

  it('formats currency according to active locale', () => {
    setLocale('vi');
    const viVnd = formatCurrency(5000000, 'VND');
    expect(viVnd).toContain('5.000.000');
    expect(viVnd).toContain('₫');

    setLocale('en');
    const enUsd = formatCurrency(1500.5, 'USD');
    expect(enUsd).toContain('1,500.50');
    expect(enUsd).toContain('$');
  });

  it('formats numbers according to active locale', () => {
    setLocale('vi');
    const viNum = formatNumber(1234567.89);
    expect(viNum).toBe('1.234.567,89');

    setLocale('en');
    const enNum = formatNumber(1234567.89);
    expect(enNum).toBe('1,234,567.89');
  });

  it('formats dates properly', () => {
    const testDate = new Date('2026-08-17T12:00:00Z');
    setLocale('vi');
    const viDate = formatDate(testDate);
    expect(viDate).toContain('2026');

    setLocale('en');
    const enDate = formatDate(testDate);
    expect(enDate).toContain('2026');
  });

  it('handles invalid or empty inputs gracefully in formatters', () => {
    expect(formatCurrency(null)).toBe('0 ₫');
    expect(formatNumber(null)).toBe('0');
    expect(formatDate(null)).toBe('');
    expect(formatDateTime(null)).toBe('');
    expect(formatRelativeTime(null)).toBe('');
  });
});
