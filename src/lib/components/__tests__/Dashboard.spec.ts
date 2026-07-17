/// <reference types="bun-types" />
import { mock } from 'bun:test';

// Mock the Tauri API core module before imports
mock.module('@tauri-apps/api/core', () => {
  return {
    invoke: (cmd: string, args?: any) => {
      if (cmd === 'get_leads') {
        return Promise.resolve([
          { id: 1, name: "Lead One", company: "Company A", status: "New", value: 1000, last_activity: "2026-07-17", owner: "Owner A", email: "a@test.com", phone: "123", about: null },
          { id: 2, name: "Lead Two", company: "Company B", status: "Contacted", value: 2000, last_activity: "2026-07-17", owner: "Owner B", email: "b@test.com", phone: "456", about: null },
          { id: 3, name: "Lead Three", company: "Company C", status: "Lost", value: 1500, last_activity: "2026-07-17", owner: "Owner C", email: "c@test.com", phone: "789", about: null }
        ]);
      }
      if (cmd === 'get_deals') {
        return Promise.resolve([
          { id: 1, lead_id: 1, title: "Deal One", value: 15000, stage: "Won", created_at: "2026-07-17" },
          { id: 2, lead_id: 2, title: "Deal Two", value: 30000, stage: "Won", created_at: "2026-07-17" },
          { id: 3, lead_id: 3, title: "Deal Three", value: 20000, stage: "Proposal", created_at: "2026-07-17" }
        ]);
      }
      return Promise.resolve();
    }
  };
});

import { render, screen } from '@testing-library/svelte';
import Dashboard from '../Dashboard.svelte';
import { expect, test } from 'bun:test';

test('renders Dashboard screen headings and initial stats', async () => {
  render(Dashboard);
  expect(screen.getByText('Sales Performance')).toBeTruthy();
  expect(screen.getByText('Revenue Target vs. Actual')).toBeTruthy();

  // Wait for the async Svelte effects to resolve and update the stats
  await new Promise((resolve) => setTimeout(resolve, 50));

  // With 2 won deals, sum = $45,000.00
  expect(screen.getByText('$45,000.00')).toBeTruthy();
  
  // Deals won = 2 and Active leads = 2
  const countElements = screen.getAllByText('2');
  expect(countElements.length).toBe(2);
});
