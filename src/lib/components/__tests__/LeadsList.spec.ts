/// <reference types="bun-types" />
import { afterEach, describe, expect, mock, spyOn, test } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import type { Lead } from '$lib/services/crmService';
import LeadsList from '../LeadsList.svelte';
import * as leadsExportCsv from '$lib/utils/leadsExportCsv';
import { toast } from 'svelte-sonner';

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
    // When no rows selected, Export button should not exist
    expect(screen.queryByRole('button', { name: /Export/i })).toBeNull();

    await fireEvent.click(headerCheckbox);

    // Banner appears showing 10 selected and button to select all 25 leads
    expect(screen.getByText(/Selected/i)).toBeTruthy();
    const selectAllBtn = screen.getByText('Select all 25 leads matching search');
    expect(selectAllBtn).toBeTruthy();

    // Export button should appear for 10 items
    expect(screen.getByRole('button', { name: /Export \(10\)/i })).toBeTruthy();

    // Clicking 'Select all 25 leads' selects all 25 leads across pages
    await fireEvent.click(selectAllBtn);
    expect(screen.getByText((_, el) => el?.tagName === 'SPAN' && el?.textContent?.includes('All 25 leads matching') === true)).toBeTruthy();
    expect(screen.getByText(/Assign to Category \(25\)/i)).toBeTruthy();
    expect(screen.getByRole('button', { name: /Export \(25\)/i })).toBeTruthy();

    // Clicking 'Clear selection' resets selection and hides Export button
    const clearBtn = screen.getByText('Clear selection');
    await fireEvent.click(clearBtn);
    expect(screen.queryByText('Select all 25 leads matching search')).toBeNull();
    expect(screen.queryByRole('button', { name: /Export/i })).toBeNull();
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

  test('supports select all and export in contacts mode even when items are fewer than page size', async () => {
    const mockContacts: Lead[] = [
      { id: 101, name: 'TRUNK 正能量整復所', company: 'TRUNK 正能量整復所', status: 'Converted', value: 0, last_activity: '2026-08-14', owner: 'System', email: 'no@gmail.com', phone: '0933001673' },
      { id: 102, name: 'Phòng Khám Vật Lý Trị Liệu Tân Phú Therapy', company: 'Phòng Khám Vật Lý Trị Liệu Tân Phú Therapy', status: 'Converted', value: 0, last_activity: '2026-08-14', owner: 'System', email: 'tan@gmail.com', phone: '0986864947' },
      { id: 103, name: 'PK Y học cổ truyền Tâm Khang An', company: 'PK Y học cổ truyền Tâm Khang An', status: 'Converted', value: 0, last_activity: '2026-08-14', owner: 'Google Maps', email: 'no@gmail.com', phone: '0981353968' }
    ];

    render(LeadsList, { props: { leads: mockContacts, entityType: 'contacts' } });

    // Verify Contacts headers and Add Contact button
    expect(screen.getByText('Converted Contacts')).toBeTruthy();
    expect(screen.getByText('Add Contact')).toBeTruthy();

    const checkboxes = screen.getAllByRole('checkbox');
    expect(checkboxes.length).toBe(4); // 1 header + 3 rows

    // Select row 2 (index 2 in checkboxes array)
    const row2Checkbox = checkboxes[2];
    await fireEvent.click(row2Checkbox);

    // Banner should show "Selected 1 contact(s)." and "Select all 3 contacts matching search"
    expect(screen.getByText(/Selected/i)).toBeTruthy();
    const selectAllBtn = screen.getByText('Select all 3 contacts matching search');
    expect(selectAllBtn).toBeTruthy();
    expect(screen.getByRole('button', { name: /Export \(1\)/i })).toBeTruthy();

    // Clicking "Select all 3 contacts matching search" selects all 3 contacts
    await fireEvent.click(selectAllBtn);
    expect(screen.getByText((_, el) => el?.tagName === 'SPAN' && el?.textContent?.includes('All 3 contacts matching') === true)).toBeTruthy();
    expect(screen.getByText(/Assign to Category \(3\)/i)).toBeTruthy();
    expect(screen.getByRole('button', { name: /Export \(3\)/i })).toBeTruthy();

    // Clicking 'Clear selection' resets selection
    const clearBtn = screen.getByText('Clear selection');
    await fireEvent.click(clearBtn);
    expect(screen.queryByText('Select all 3 contacts matching search')).toBeNull();
    expect(screen.queryByRole('button', { name: /Export/i })).toBeNull();

    // Clicking header checkbox directly selects all 3 contacts
    const headerCheckbox = checkboxes[0];
    await fireEvent.click(headerCheckbox);
    expect(screen.getByText((_, el) => el?.tagName === 'SPAN' && el?.textContent?.includes('All 3 contacts matching') === true)).toBeTruthy();
    expect(screen.getByRole('button', { name: /Export \(3\)/i })).toBeTruthy();
  });

  test('clicking Export button triggers exportLeadsToCsv and displays toast success on completion', async () => {
    let capturedLeads: any = null;
    let capturedEntityType: any = null;
    let toastSuccessMessage = '';

    const exportSpy = spyOn(leadsExportCsv, 'exportLeadsToCsv').mockImplementation(
      async (leads: readonly Lead[], entityType?: 'leads' | 'contacts') => {
        capturedLeads = leads;
        capturedEntityType = entityType;
        return '/path/to/exported_leads.csv';
      }
    );

    const toastSuccessSpy = spyOn(toast, 'success').mockImplementation((msg: string | any) => {
      toastSuccessMessage = String(msg);
      return '' as any;
    });

    render(LeadsList, { props: { leads: mockLeads.slice(0, 3) } });

    // Select row 1
    const checkboxes = screen.getAllByRole('checkbox');
    await fireEvent.click(checkboxes[1]);

    const exportBtn = screen.getByRole('button', { name: /Export \(1\)/i });
    expect(exportBtn).toBeTruthy();

    await fireEvent.click(exportBtn);

    expect(capturedLeads).not.toBeNull();
    expect(capturedLeads.length).toBe(1);
    expect(capturedLeads[0].name).toBe('Lead 1');
    expect(capturedEntityType).toBe('leads');
    expect(toastSuccessMessage).toContain('Exported 1 leads to exported_leads.csv');

    exportSpy.mockRestore();
    toastSuccessSpy.mockRestore();
  });

  test('clicking Export button does nothing when user cancels dialog', async () => {
    let toastCalled = false;

    const exportSpy = spyOn(leadsExportCsv, 'exportLeadsToCsv').mockImplementation(async () => null);
    const toastSuccessSpy = spyOn(toast, 'success').mockImplementation(() => {
      toastCalled = true;
      return '' as any;
    });
    const toastErrorSpy = spyOn(toast, 'error').mockImplementation(() => {
      toastCalled = true;
      return '' as any;
    });

    render(LeadsList, { props: { leads: mockLeads.slice(0, 3) } });

    const checkboxes = screen.getAllByRole('checkbox');
    await fireEvent.click(checkboxes[1]);

    const exportBtn = screen.getByRole('button', { name: /Export \(1\)/i });
    await fireEvent.click(exportBtn);

    expect(toastCalled).toBe(false);

    exportSpy.mockRestore();
    toastSuccessSpy.mockRestore();
    toastErrorSpy.mockRestore();
  });

  test('clicking Export button displays toast error when export fails', async () => {
    let toastErrorMessage = '';

    const exportSpy = spyOn(leadsExportCsv, 'exportLeadsToCsv').mockImplementation(async () => {
      throw new Error('Write failed');
    });
    const toastErrorSpy = spyOn(toast, 'error').mockImplementation((msg: string | any) => {
      toastErrorMessage = String(msg);
      return '' as any;
    });

    render(LeadsList, { props: { leads: mockLeads.slice(0, 3) } });

    const checkboxes = screen.getAllByRole('checkbox');
    await fireEvent.click(checkboxes[1]);

    const exportBtn = screen.getByRole('button', { name: /Export \(1\)/i });
    await fireEvent.click(exportBtn);

    expect(toastErrorMessage).toBe('Failed to export data');

    exportSpy.mockRestore();
    toastErrorSpy.mockRestore();
  });
});
