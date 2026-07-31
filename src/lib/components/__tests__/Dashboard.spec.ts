/// <reference types="bun-types" />
import { afterEach, mock } from 'bun:test';

let leadsList = [
  { id: 1, name: "Lead One", company: "Company A", status: "New", value: 1000, last_activity: "2026-07-17", owner: "Owner A", email: "a@test.com", phone: "123", about: null },
  { id: 2, name: "Lead Two", company: "Company B", status: "Contacted", value: 2000, last_activity: "2026-07-17", owner: "Owner B", email: "b@test.com", phone: "456", about: null },
  { id: 3, name: "Lead Three", company: "Company C", status: "Lost", value: 1500, last_activity: "2026-07-17", owner: "Owner C", email: "c@test.com", phone: "789", about: null }
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
        return Promise.resolve([
          { id: 1, lead_id: 1, title: "Deal One", value: 15000, stage: "Won", created_at: "2026-07-17" },
          { id: 2, lead_id: 2, title: "Deal Two", value: 30000, stage: "Won", created_at: "2026-07-17" },
          { id: 3, lead_id: 3, title: "Deal Three", value: 20000, stage: "Proposal", created_at: "2026-07-17" }
        ]);
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

import { cleanup, render, screen } from '@testing-library/svelte';
import Dashboard from '../Dashboard.svelte';
import { notifyLeadsChanged } from '$lib/services/crmService';
import { expect, test } from 'bun:test';

afterEach(() => {
  cleanup();
});

test('renders Dashboard screen headings and initial stats', async () => {
  render(Dashboard);
  expect(screen.getByText('Sales Performance')).toBeTruthy();
  expect(screen.getByText('Revenue Target vs. Actual')).toBeTruthy();

  // Wait for the async Svelte effects to resolve and update the stats
  const revenueElement = await screen.findByText('$45,000.00');
  expect(revenueElement).toBeTruthy();
  
  // Deals won = 2 and Active leads = 2
  const countElements = screen.getAllByText('2');
  expect(countElements.length).toBe(2);
});

test('updates active leads count in real-time when notifyLeadsChanged is fired', async () => {
  render(Dashboard, { props: { activeTab: 'dashboard' } });

  // Wait for initial render
  await screen.findByText('$45,000.00');

  // Simulate scraper adding 3 new leads to the database
  leadsList = [
    ...leadsList,
    { id: 4, name: "Lead Four", company: "Company D", status: "New", value: 1000, last_activity: "2026-07-30", owner: "Owner D", email: "d@test.com", phone: "111", about: null },
    { id: 5, name: "Lead Five", company: "Company E", status: "New", value: 1000, last_activity: "2026-07-30", owner: "Owner E", email: "e@test.com", phone: "222", about: null },
    { id: 6, name: "Lead Six", company: "Company F", status: "New", value: 1000, last_activity: "2026-07-30", owner: "Owner F", email: "f@test.com", phone: "333", about: null }
  ];

  // Trigger lead notification
  notifyLeadsChanged();

  // Wait for debounced reload to update active leads count from 2 to 5
  const updatedCountElement = await screen.findByText('5');
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
