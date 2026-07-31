/// <reference types="bun-types" />
import { afterEach, beforeEach, expect, mock, test } from 'bun:test';

let mode: 'retry-leads' | 'category-leads-error' | 'success' = 'retry-leads';
let getCategoriesCallCount = 0;
let getLeadsCallCount = 0;
let getCategoryLeadsCallCount = 0;

mock.module('@tauri-apps/api/core', () => ({
  invoke: async (command: string) => {
    if (command === 'get_categories') {
      getCategoriesCallCount += 1;
      return mode === 'category-leads-error'
        ? [{ id: 1, name: 'Audit Category', color: '#000000' }]
        : [];
    }

    if (command === 'get_leads') {
      getLeadsCallCount += 1;
      if (mode === 'retry-leads' && getLeadsCallCount === 1) {
        throw new Error('get_leads failed');
      }
      return [];
    }

    if (command === 'get_leads_by_category') {
      getCategoryLeadsCallCount += 1;
      throw new Error('get_leads_by_category failed');
    }

    return [];
  }
}));

mock.module('@tauri-apps/api/event', () => ({
  listen: async () => () => {}
}));

import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import CategoryManager from '../CategoryManager.svelte';
import { notifyLeadsChanged } from '$lib/services/crmService';

beforeEach(() => {
  mode = 'retry-leads';
  getCategoriesCallCount = 0;
  getLeadsCallCount = 0;
  getCategoryLeadsCallCount = 0;
});

afterEach(() => {
  cleanup();
});

test('retries all category data after a lead load failure and tab revisit', async () => {
  const view = render(CategoryManager, { props: { activeTab: 'categories' } });
  await new Promise((resolve) => setTimeout(resolve, 100));

  expect(getCategoriesCallCount).toBe(1);
  expect(getLeadsCallCount).toBe(1);

  await view.rerender({ activeTab: 'dashboard' });
  await view.rerender({ activeTab: 'categories' });
  await new Promise((resolve) => setTimeout(resolve, 100));

  expect(getCategoriesCallCount).toBe(2);
  expect(getLeadsCallCount).toBe(2);
});

test('handles category lead load errors without an unhandled effect rejection', async () => {
  mode = 'category-leads-error';
  render(CategoryManager, { props: { activeTab: 'categories' } });

  const categoryButton = await screen.findByRole('button', { name: 'Audit Category' });
  await fireEvent.click(categoryButton);
  await new Promise((resolve) => setTimeout(resolve, 100));

  expect(getCategoryLeadsCallCount).toBe(1);
  expect(screen.getByText('get_leads_by_category failed')).toBeTruthy();
});

test('does not re-fetch category data on clean tab switch without background events', async () => {
  mode = 'success';
  getCategoriesCallCount = 0;
  getLeadsCallCount = 0;

  const view = render(CategoryManager, { props: { activeTab: 'categories' } });
  await new Promise((resolve) => setTimeout(resolve, 100));

  const initialCategoriesCount = getCategoriesCallCount;
  expect(initialCategoriesCount).toBeGreaterThanOrEqual(1);

  // Clean tab switch away and back
  await view.rerender({ activeTab: 'dashboard' });
  await view.rerender({ activeTab: 'categories' });
  await new Promise((resolve) => setTimeout(resolve, 100));

  // Should NOT re-fetch because no background event occurred
  expect(getCategoriesCallCount).toBe(initialCategoriesCount);
});

test('re-fetches category data on tab revisit if background event occurred and tab was switched within 300ms', async () => {
  mode = 'success';
  getCategoriesCallCount = 0;
  getLeadsCallCount = 0;

  const view = render(CategoryManager, { props: { activeTab: 'categories' } });
  await new Promise((resolve) => setTimeout(resolve, 100));

  const initialCount = getCategoriesCallCount;

  // Trigger background event while tab is open -> starts 300ms reloadTimer
  notifyLeadsChanged();

  // Switch tab immediately (<300ms) while reloadTimer is running
  await view.rerender({ activeTab: 'dashboard' });

  // Revisit tab after 100ms
  await view.rerender({ activeTab: 'categories' });
  await new Promise((resolve) => setTimeout(resolve, 100));

  // MUST re-fetch because reloadTimer was cancelled and marked isStale = true
  expect(getCategoriesCallCount).toBeGreaterThan(initialCount);
});
