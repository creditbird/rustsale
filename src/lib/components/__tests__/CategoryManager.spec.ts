/// <reference types="bun-types" />
import { beforeEach, expect, mock, test } from 'bun:test';

let mode: 'retry-leads' | 'category-leads-error' = 'retry-leads';
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

import { fireEvent, render, screen } from '@testing-library/svelte';
import CategoryManager from '../CategoryManager.svelte';

beforeEach(() => {
  mode = 'retry-leads';
  getCategoriesCallCount = 0;
  getLeadsCallCount = 0;
  getCategoryLeadsCallCount = 0;
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
