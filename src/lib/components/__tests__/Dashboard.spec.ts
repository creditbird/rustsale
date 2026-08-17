/// <reference types="bun-types" />
import { afterEach, mock } from 'bun:test';

let leadsList: any[] = [
  { id: 1, name: "Lead One", company: "Company A", status: "New", value: 1000, last_activity: "2026-07-17", owner: "Owner A", email: "a@test.com", phone: "123", about: null, currency: "VND" },
  { id: 2, name: "Lead Two", company: "Company B", status: "Contacted", value: 2000, last_activity: "2026-07-17", owner: "Owner B", email: "b@test.com", phone: "456", about: null, currency: "VND" },
  { id: 3, name: "Lead Three", company: "Company C", status: "Lost", value: 1500, last_activity: "2026-07-17", owner: "Owner C", email: "c@test.com", phone: "789", about: null, currency: "VND" }
];
let dealsList: any[] = [
  { id: 1, lead_id: 1, title: "Deal One", value: 15000, stage: "Won", created_at: "2026-07-17", currency: "VND" },
  { id: 2, lead_id: 2, title: "Deal Two", value: 30000, stage: "Won", created_at: "2026-07-17", currency: "VND" },
  { id: 3, lead_id: 3, title: "Deal Three", value: 20000, stage: "Proposal", created_at: "2026-07-17", currency: "VND" }
];
let getLeadsInvokeCount = 0;

// Mock the Tauri API modules before imports
mock.module('@tauri-apps/api/core', () => {
  return {
    invoke: (cmd: string) => {
      if (cmd === 'get_leads') {
        getLeadsInvokeCount += 1;
        return Promise.resolve(leadsList);
      }
      if (cmd === 'get_deals') {
        return Promise.resolve(dealsList);
      }
      if (cmd === 'get_recent_activities') {
        return Promise.resolve([]);
      }
      return Promise.resolve();
    }
  };
});

mock.module('@tauri-apps/api/event', () => ({
  listen: async () => () => {}
}));

import { cleanup, render, screen, fireEvent } from '@testing-library/svelte';
import Dashboard from '../Dashboard.svelte';
import { notifyLeadsChanged } from '$lib/services/crmService';
import { expect, test, beforeEach } from 'bun:test';
import { setLocale, formatCurrency, formatCompactCurrency, setBaseCurrency, setExchangeRate } from '$lib/i18n';

beforeEach(() => {
  setLocale('en');
  setBaseCurrency('VND');
  setExchangeRate(25400);
  dealsList = [
    { id: 1, lead_id: 1, title: "Deal One", value: 15000, stage: "Won", created_at: "2026-07-17", currency: "VND" },
    { id: 2, lead_id: 2, title: "Deal Two", value: 30000, stage: "Won", created_at: "2026-07-17", currency: "VND" },
    { id: 3, lead_id: 3, title: "Deal Three", value: 20000, stage: "Proposal", created_at: "2026-07-17", currency: "VND" }
  ];
  leadsList = [
    { id: 1, name: "Lead One", company: "Company A", status: "New", value: 1000, last_activity: "2026-07-17", owner: "Owner A", email: "a@test.com", phone: "123", about: null, currency: "VND" },
    { id: 2, name: "Lead Two", company: "Company B", status: "Contacted", value: 2000, last_activity: "2026-07-17", owner: "Owner B", email: "b@test.com", phone: "456", about: null, currency: "VND" },
    { id: 3, name: "Lead Three", company: "Company C", status: "Lost", value: 1500, last_activity: "2026-07-17", owner: "Owner C", email: "c@test.com", phone: "789", about: null, currency: "VND" }
  ];
});

afterEach(() => {
  cleanup();
  localStorage.clear();
});

test('renders Dashboard screen headings and initial stats with accurate KPI labels', async () => {
  render(Dashboard);
  expect(screen.getByText('Sales Performance')).toBeTruthy();
  expect(screen.getByText('Revenue Target vs. Actual')).toBeTruthy();

  // Verify KPI Card labels in English
  expect(screen.getByText('Total Revenue Generated')).toBeTruthy();
  expect(screen.getByText('Deals Won')).toBeTruthy();
  expect(screen.getByText('Active Leads')).toBeTruthy();

  // Wait for the async Svelte effects to resolve and update the stats
  const expectedRevenue = formatCurrency(45000, 'VND');
  const revenueElements = await screen.findAllByText(expectedRevenue, {}, { timeout: 3000 });
  expect(revenueElements.length).toBeGreaterThanOrEqual(1);
  
  // Deals won = 2 and Active leads = 2
  const countElements = await screen.findAllByText('2', {}, { timeout: 3000 });
  expect(countElements.length).toBeGreaterThanOrEqual(2);
});

test('renders Dashboard with Vietnamese KPI labels when locale is vi', async () => {
  setLocale('vi');
  render(Dashboard);

  expect(screen.getByText('Tổng Doanh Thu Thực Tế')).toBeTruthy();
  expect(screen.getByText('Giao Dịch Thành Công')).toBeTruthy();
  expect(screen.getByText('Lead Đang Xử Lý')).toBeTruthy();
});

test('updates active leads count in real-time when notifyLeadsChanged is fired', async () => {
  render(Dashboard, { props: { activeTab: 'dashboard' } });

  // Wait for initial render
  const expectedRevenue = formatCurrency(45000, 'VND');
  const initialRevenueElements = await screen.findAllByText(expectedRevenue, {}, { timeout: 3000 });
  expect(initialRevenueElements.length).toBeGreaterThanOrEqual(1);

  // Simulate scraper adding 3 new leads to the database
  leadsList = [
    ...leadsList,
    { id: 4, name: "Lead Four", company: "Company D", status: "New", value: 1000, last_activity: "2026-07-30", owner: "Owner D", email: "d@test.com", phone: "111", about: null },
    { id: 5, name: "Lead Five", company: "Company E", status: "New", value: 1000, last_activity: "2026-07-30", owner: "Owner E", email: "e@test.com", phone: "222", about: null },
    { id: 6, name: "Lead Six", company: "Company F", status: "New", value: 1000, last_activity: "2026-07-30", owner: "Owner F", email: "f@test.com", phone: "333", about: null }
  ];

  // Trigger lead notification
  notifyLeadsChanged();
  await new Promise((resolve) => setTimeout(resolve, 400));

  // Wait for debounced reload to update active leads count from 2 to 5
  const updatedCountElement = await screen.findByText('5', {}, { timeout: 4000 });
  expect(updatedCountElement).toBeTruthy();
});

test('handles empty dataset without infinite IPC loop probe', async () => {
  leadsList = [];
  getLeadsInvokeCount = 0;

  render(Dashboard, { props: { activeTab: 'dashboard' } });

  // Wait for async Svelte render
  await new Promise((resolve) => setTimeout(resolve, 200));

  // Verify heading exists
  expect(screen.getAllByText('Sales Performance').length).toBeGreaterThan(0);

  // Should fetch at most twice on mount/initial effect, NOT loop infinitely (which would be 10-50+ calls)
  expect(getLeadsInvokeCount).toBeLessThanOrEqual(2);
});

test('does not re-fetch dashboard data on clean tab switch without background events', async () => {
  leadsList = [
    { id: 1, name: "Lead One", company: "Company A", status: "New", value: 1000, last_activity: "2026-07-17", owner: "Owner A", email: "a@test.com", phone: "123", about: null }
  ];
  getLeadsInvokeCount = 0;

  const view = render(Dashboard, { props: { activeTab: 'dashboard' } });
  await new Promise((resolve) => setTimeout(resolve, 100));

  const initialFetchCount = getLeadsInvokeCount;

  // Clean tab switch away and back
  await view.rerender({ activeTab: 'leads' });
  await view.rerender({ activeTab: 'dashboard' });
  await new Promise((resolve) => setTimeout(resolve, 100));

  // Should NOT re-fetch
  expect(getLeadsInvokeCount).toBe(initialFetchCount);
});

test('re-fetches dashboard data on tab revisit if background event occurred and tab switched within 300ms', async () => {
  leadsList = [
    { id: 1, name: "Lead One", company: "Company A", status: "New", value: 1000, last_activity: "2026-07-17", owner: "Owner A", email: "a@test.com", phone: "123", about: null }
  ];
  getLeadsInvokeCount = 0;

  const view = render(Dashboard, { props: { activeTab: 'dashboard' } });
  await new Promise((resolve) => setTimeout(resolve, 100));

  const initialFetchCount = getLeadsInvokeCount;

  // Trigger background event -> starts 300ms reloadTimer
  notifyLeadsChanged();

  // Switch tab immediately (<300ms)
  await view.rerender({ activeTab: 'leads' });

  // Revisit tab after 100ms
  await view.rerender({ activeTab: 'dashboard' });
  await new Promise((resolve) => setTimeout(resolve, 250));

  // MUST re-fetch because reloadTimer was cancelled mid-flight and marked isStale = true
  expect(getLeadsInvokeCount).toBeGreaterThan(initialFetchCount);
});

test('migrates legacy USD-scale localStorage targets (<1M) to canonical VND baseline', async () => {
  localStorage.setItem('rustsale_target_revenue', '120000');
  localStorage.setItem('rustsale_stretch_revenue', '150000');

  render(Dashboard, { props: { activeTab: 'dashboard' } });

  // Open Edit Target modal and verify legacy < 1M targets were sanitized to VND baseline (120M / 150M)
  const editTargetBtn = screen.getByRole('button', { name: /Edit Target/i });
  await fireEvent.click(editTargetBtn);

  const targetInput = screen.getByLabelText(/Target Revenue/i) as HTMLInputElement;
  const stretchInput = screen.getByLabelText(/Stretch Goal/i) as HTMLInputElement;

  expect(Number(targetInput.value)).toBe(120000000);
  expect(Number(stretchInput.value)).toBe(150000000);
});

test('persists updated VND targets using versioned keys in localStorage', async () => {
  render(Dashboard, { props: { activeTab: 'dashboard' } });

  // Open Edit Target modal
  const editTargetBtn = screen.getByRole('button', { name: /Edit Target/i });
  await fireEvent.click(editTargetBtn);

  // Set new target and stretch goal
  const targetInput = screen.getByLabelText(/Target Revenue/i) as HTMLInputElement;
  const stretchInput = screen.getByLabelText(/Stretch Goal/i) as HTMLInputElement;

  await fireEvent.input(targetInput, { target: { value: '200000000' } });
  await fireEvent.input(stretchInput, { target: { value: '250000000' } });

  // Save targets
  const saveBtn = screen.getByRole('button', { name: /Save Target/i });
  await fireEvent.submit(saveBtn.closest('form')!);

  // Verify localStorage keys were versioned and persisted
  expect(localStorage.getItem('rustsale_target_revenue_v2')).toBe('200000000');
  expect(localStorage.getItem('rustsale_stretch_revenue_v2')).toBe('250000000');
});

test('accurately calculates target completion % when Base Currency is USD without dimension error', async () => {
  setBaseCurrency('USD');
  setExchangeRate(25400);

  // 127,000,000 VND target ($5,000 USD at 25400)
  localStorage.setItem('rustsale_target_revenue_v2', '127000000');
  localStorage.setItem('rustsale_stretch_revenue_v2', '254000000');

  // Won deal of $2,500 USD (50% of target)
  dealsList = [
    { id: 10, lead_id: 1, title: "USD Won Deal", value: 2500, currency: "USD", stage: "Won", created_at: "2026-07-15" }
  ];

  render(Dashboard, { props: { activeTab: 'dashboard' } });

  // Completion rate should be exactly 50% (not 0.002% from raw VND division)
  const completionElements = await screen.findAllByText('50%', {}, { timeout: 3000 });
  expect(completionElements.length).toBeGreaterThanOrEqual(1);

  // Formatted converted target should display ~$5,000.00 (not $127,000,000)
  expect(screen.getByText(/\/ \$5,000\.00 Target/i)).toBeTruthy();
});

test('accurately sums mixed-currency won deals in monthly trend data with FX conversion', async () => {
  setBaseCurrency('VND');
  setExchangeRate(25400);

  // 1 VND won deal (25,400,000 VND) + 1 USD won deal ($1,000 = 25,400,000 VND)
  dealsList = [
    { id: 1, lead_id: 1, title: "VND Deal", value: 25400000, currency: "VND", stage: "Won", created_at: "2026-07-10" },
    { id: 2, lead_id: 2, title: "USD Deal", value: 1000, currency: "USD", stage: "Won", created_at: "2026-07-15" }
  ];

  render(Dashboard, { props: { activeTab: 'dashboard' } });

  // Total revenue is 50,800,000 VND (not 25,401,000 raw)
  const expectedTotalStr = formatCurrency(50800000, 'VND');
  const revenueElements = await screen.findAllByText(expectedTotalStr, {}, { timeout: 3000 });
  expect(revenueElements.length).toBeGreaterThanOrEqual(1);

  // Jul 2026 trend data displays 50,800,000 VND
  expect(screen.getByText('Jul 2026')).toBeTruthy();
  expect(screen.getAllByText(expectedTotalStr).length).toBeGreaterThanOrEqual(2);
});

test('ranks top active deals by converted base currency value', async () => {
  setBaseCurrency('VND');
  setExchangeRate(25400);

  // Deal A: $5,000 USD (~127,000,000 VND)
  // Deal B: 100,000,000 VND
  dealsList = [
    { id: 1, lead_id: 1, title: "VND Deal 100M", value: 100000000, currency: "VND", stage: "Demo", created_at: "2026-07-10" },
    { id: 2, lead_id: 2, title: "USD Deal 5K", value: 5000, currency: "USD", stage: "Proposal", created_at: "2026-07-10" }
  ];

  render(Dashboard, { props: { activeTab: 'dashboard' } });

  // Wait for deals to load
  await screen.findByText('USD Deal 5K', {}, { timeout: 3000 });

  const dealTitles = screen.getAllByRole('heading', { level: 4 }).map(el => el.textContent);
  const usdIndex = dealTitles.indexOf('USD Deal 5K');
  const vndIndex = dealTitles.indexOf('VND Deal 100M');

  // $5,000 USD (127m VND) MUST rank higher than 100,000,000 VND
  expect(usdIndex).toBeLessThan(vndIndex);
});

test('returns zero metrics when selected period has zero matching deals and excludes missing/invalid created_at', async () => {
  setBaseCurrency('VND');
  setExchangeRate(25400);

  // All deals are from 2025 or have missing/invalid created_at dates
  dealsList = [
    { id: 1, lead_id: 1, title: "2025 Deal", value: 100000000, currency: "VND", stage: "Won", created_at: "2025-05-10" },
    { id: 2, lead_id: 2, title: "Missing Date Deal", value: 50000000, currency: "VND", stage: "Won", created_at: null },
    { id: 3, lead_id: 3, title: "Invalid Date Deal", value: 20000000, currency: "VND", stage: "Won", created_at: "invalid-date-string" }
  ];

  render(Dashboard, { props: { activeTab: 'dashboard' } });

  // Default selected period is 'Q3 2026'
  // When no deals match Q3 2026, activeDeals MUST be empty (0 deals), totalRevenue MUST be 0 ₫
  // It MUST NOT fall back to the 2025 historical deals (100,000,000 VND)
  const zeroRevenueStr = formatCurrency(0, 'VND');
  const revenueElements = await screen.findAllByText(zeroRevenueStr, {}, { timeout: 3000 });
  expect(revenueElements.length).toBeGreaterThanOrEqual(1);

  // Deals count badge and stage counts should show 0 deals
  const zeroDealsElements = screen.getAllByText('0 deals');
  expect(zeroDealsElements.length).toBeGreaterThanOrEqual(1);
  // Deals won should be 0
  const zeroCounts = screen.getAllByText('0');
  expect(zeroCounts.length).toBeGreaterThanOrEqual(1);
});
