/// <reference types="bun-types" />
import { afterEach, describe, expect, test } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import type { Lead } from '$lib/services/crmService';
import LeadsList from '../LeadsList.svelte';

describe('LeadsList Component & Selection Banner', () => {
  const mockLeads: Lead[] = Array.from({ length: 25 }, (_, i) => ({
    id: i + 1,
    name: `Lead ${i + 1}`,
    company: `Company ${i + 1}`,
    status: 'New',
    value: 1000 * (i + 1),
    last_activity: '2026-07-31',
    owner: 'Self',
    email: `lead${i + 1}@example.com`,
    phone: `09000000${i + 1}`
  }));

  afterEach(() => {
    cleanup();
  });

  test('renders LeadsList with mock data and supports select all page and select all filtered', async () => {
    render(LeadsList, { props: { leads: mockLeads } });

    // Table header and first page (10 leads) should be present
    expect(screen.getByText('Lead 1')).toBeTruthy();
    expect(screen.getByText('Lead 10')).toBeTruthy();
    expect(screen.queryByText('Lead 11')).toBeNull(); // On page 2

    // Ticking the top header checkbox selects the current page (10 leads)
    const checkboxes = screen.getAllByRole('checkbox');
    expect(checkboxes.length).toBeGreaterThan(0);
    const headerCheckbox = checkboxes[0];

    await fireEvent.click(headerCheckbox);

    // Banner appears showing 10 selected and button to select all 25 leads
    expect(screen.getByText(/Selected/i)).toBeTruthy();
    const selectAllBtn = screen.getByText('Select all 25 leads matching search');
    expect(selectAllBtn).toBeTruthy();

    // Clicking 'Select all 25 leads' selects all 25 leads across pages
    await fireEvent.click(selectAllBtn);
    expect(screen.getByText((_, el) => el?.tagName === 'SPAN' && el?.textContent?.includes('All 25 leads matching') === true)).toBeTruthy();
    expect(screen.getByText(/Assign to Category \(25\)/i)).toBeTruthy();

    // Clicking 'Clear selection' resets selection
    const clearBtn = screen.getByText('Clear selection');
    await fireEvent.click(clearBtn);
    expect(screen.queryByText('Select all 25 leads matching search')).toBeNull();
  });

  test('correctly calculates pagination and page-level selection logic', () => {
    const pageSize = 10;
    const currentPage = 1;
    const paginatedLeads = mockLeads.slice((currentPage - 1) * pageSize, currentPage * pageSize);

    expect(paginatedLeads.length).toBe(10);
    expect(paginatedLeads[0].id).toBe(1);
    expect(paginatedLeads[9].id).toBe(10);

    const selectedRows = new Set<number>();
    paginatedLeads.forEach((l) => selectedRows.add(l.id!));

    const isCurrentPageAllSelected = paginatedLeads.length > 0 && paginatedLeads.every((l) => selectedRows.has(l.id!));
    const isAllFilteredSelected = mockLeads.length > 0 && selectedRows.size === mockLeads.length;

    expect(isCurrentPageAllSelected).toBe(true);
    expect(isAllFilteredSelected).toBe(false);
    expect(selectedRows.size).toBe(10);
  });
});
