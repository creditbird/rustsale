import { describe, it, expect, beforeEach } from 'bun:test';
import {
  m,
  setLocale,
  getLocale,
  currencyState,
  getBaseCurrency,
  setBaseCurrency,
  getExchangeRate,
  setExchangeRate,
  DEFAULT_USD_VND_RATE,
  convertCurrency
} from '../index';
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
    setBaseCurrency('VND');
    setExchangeRate(DEFAULT_USD_VND_RATE);
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

  it('correctly converts between VND and USD based on exchange rate', () => {
    // USD -> VND (100 USD at 25,400 = 2,540,000 VND)
    expect(convertCurrency(100, 'USD', 'VND', 25400)).toBe(2540000);
    // VND -> USD (2,540,000 VND at 25,400 = 100 USD)
    expect(convertCurrency(2540000, 'VND', 'USD', 25400)).toBe(100);
    // Same currency identity
    expect(convertCurrency(5000, 'USD', 'USD')).toBe(5000);
    expect(convertCurrency(1000000, 'VND', 'VND')).toBe(1000000);
    // Edge cases: null, undefined, NaN, 0, negative
    expect(convertCurrency(null, 'USD', 'VND')).toBe(0);
    expect(convertCurrency(undefined, 'USD', 'VND')).toBe(0);
    expect(convertCurrency(NaN, 'USD', 'VND')).toBe(0);
    expect(convertCurrency(0, 'USD', 'VND')).toBe(0);
    // Custom exchange rate
    expect(convertCurrency(200, 'USD', 'VND', 25000)).toBe(5000000);
    expect(convertCurrency(5000000, 'VND', 'USD', 25000)).toBe(200);
  });

  it('manages Base Currency and Exchange Rate state reactively and rolls back on failure', async () => {
    expect(getBaseCurrency()).toBe('VND');
    expect(getExchangeRate()).toBe(25400);

    await setBaseCurrency('USD');
    expect(getBaseCurrency()).toBe('USD');
    expect(currencyState.baseCurrency).toBe('USD');

    await setExchangeRate(26000);
    expect(getExchangeRate()).toBe(26000);
    expect(currencyState.exchangeRate).toBe(26000);

    await currencyState.resetExchangeRate();
    expect(getExchangeRate()).toBe(25400);

    // Invalid inputs are rejected with Error
    // @ts-expect-error invalid currency test
    await expect(setBaseCurrency('EUR')).rejects.toThrow();
    expect(getBaseCurrency()).toBe('USD');

    await expect(setExchangeRate(-100)).rejects.toThrow();
    expect(getExchangeRate()).toBe(25400);
    await expect(setExchangeRate(NaN)).rejects.toThrow();
    expect(getExchangeRate()).toBe(25400);

    // Rollback test: simulate backend sync failure
    const originalSync = currencyState.syncToBackend;
    currencyState.syncToBackend = async () => {
      throw new Error('Simulated backend persistence failure');
    };

    await expect(setBaseCurrency('VND')).rejects.toThrow('Simulated backend persistence failure');
    expect(currencyState.baseCurrency).toBe('USD'); // Rolled back to USD

    await expect(setExchangeRate(30000)).rejects.toThrow('Simulated backend persistence failure');
    expect(currencyState.exchangeRate).toBe(25400); // Rolled back to 25400

    currencyState.syncToBackend = originalSync;
  });

  it('awaits backend hydration before user mutations to avoid clobbering authoritative sibling settings with stale cache', async () => {
    const { CurrencyStore } = await import('../state.svelte.js');
    
    // Set stale localStorage cache before creating fresh store
    localStorage.setItem('rustsale_base_currency', 'VND');
    localStorage.setItem('rustsale_usd_vnd_exchange_rate', '25400');

    // Create a new store instance with a deferred backend hydration promise
    const testStore = new CurrencyStore();
    
    /** @type {((val: any) => void) | null} */
    let resolveHydration = null;
    const hydrationPromise = new Promise((resolve) => {
      resolveHydration = resolve;
    });

    // Mock backend sync with authoritative settings in SQLite (rate = 26000, different from stale cache 25400)
    testStore.syncFromBackend = async () => {
      await hydrationPromise;
      testStore.baseCurrency = 'VND';
      testStore.exchangeRate = 26000;
      testStore.isHydrated = true;
    };
    testStore.ready = testStore.syncFromBackend();

    /** @type {Array<{baseCurrency: string, exchangeRate: number}>} */
    const syncCalls: Array<{baseCurrency: string, exchangeRate: number}> = [];
    testStore.syncToBackend = async (baseCurrency, exchangeRate) => {
      syncCalls.push({ baseCurrency, exchangeRate });
    };

    // User triggers a mutation while hydration is still in-flight
    const mutationPromise = testStore.setBaseCurrency('USD');

    // Mutation should not have completed sync yet because it is awaiting hydration
    expect(syncCalls.length).toBe(0);

    // Resolve backend hydration with authoritative rate 26000
    resolveHydration!({});
    await mutationPromise;

    // Verify final state: user's baseCurrency is 'USD', and authoritative exchangeRate 26000 is preserved
    expect(testStore.baseCurrency).toBe('USD');
    expect(testStore.exchangeRate).toBe(26000);

    // Verify that syncToBackend was invoked with the authoritative rate (26000) rather than stale cache (25400)
    expect(syncCalls.length).toBe(1);
    expect(syncCalls[0]).toEqual({ baseCurrency: 'USD', exchangeRate: 26000 });
  });

  it('serializes concurrent currency and exchange rate mutations safely', async () => {
    const { CurrencyStore } = await import('../state.svelte.js');
    const testStore = new CurrencyStore();
    await testStore.ensureReady();

    /** @type {Array<{baseCurrency: string, exchangeRate: number}>} */
    const syncCalls: Array<{baseCurrency: string, exchangeRate: number}> = [];
    testStore.syncToBackend = async (baseCurrency, exchangeRate) => {
      // Simulate minor async latency
      await new Promise(r => setTimeout(r, 5));
      syncCalls.push({ baseCurrency, exchangeRate });
    };

    // Launch concurrent mutations
    await Promise.all([
      testStore.setBaseCurrency('USD'),
      testStore.setExchangeRate(27500)
    ]);

    expect(testStore.baseCurrency).toBe('USD');
    expect(testStore.exchangeRate).toBe(27500);
    expect(syncCalls.length).toBe(2);
    // Final sync call contains both updated values
    expect(syncCalls[1]).toEqual({ baseCurrency: 'USD', exchangeRate: 27500 });
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
