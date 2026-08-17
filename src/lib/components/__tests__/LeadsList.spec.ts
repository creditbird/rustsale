import { afterEach, beforeEach, describe, expect, mock, spyOn, test } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import type { Lead } from '$lib/services/crmService';
import LeadsList from '../LeadsList.svelte';
import * as leadsExportCsv from '$lib/utils/leadsExportCsv';
import { toast } from 'svelte-sonner';
import { setLocale, convertCurrency, type Currency } from '$lib/i18n';

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

  beforeEach(() => {
    setLocale('en');
  });

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

    expect(toastErrorMessage).toBe('Failed to export leads');

    exportSpy.mockRestore();
    toastErrorSpy.mockRestore();
  });

  test('resets selection and pagination when search query or filter changes', async () => {
    render(LeadsList, { props: { leads: mockLeads } });

    // Select row 1 on page 1
    const checkboxes = screen.getAllByRole('checkbox');
    await fireEvent.click(checkboxes[1]);
    expect(screen.getByText(/Selected 1/i)).toBeTruthy();

    // Type into search input
    const searchInput = screen.getByPlaceholderText('Search name, company, email, owner...');
    await fireEvent.input(searchInput, { target: { value: 'Lead 2' } });

    // Selection should be cleared to prevent acting on hidden leads
    expect(screen.queryByText(/Selected/i)).toBeNull();
  });

  test('strict isAllFilteredSelected does not trigger when selected IDs do not match filtered leads', () => {
    const filteredLeads = [mockLeads[1]]; // Lead 2
    const selectedRows = new Set([mockLeads[0].id!]); // Lead 1 is selected, but not in filteredLeads

    const isAllFilteredSelected = filteredLeads.length > 0 && filteredLeads.every((l) => selectedRows.has(l.id!));
    expect(isAllFilteredSelected).toBe(false);
  });

  test('renders Vietnamese translated labels when locale is vi', async () => {
    setLocale('vi');
    render(LeadsList, { props: { leads: mockLeads } });

    // Verify Vietnamese translated buttons and headers
    expect(screen.getByText('Thêm Lead')).toBeTruthy();
    expect(screen.getByText('Quản lý Khách hàng tiềm năng')).toBeTruthy();
    expect(screen.getByPlaceholderText('Tìm theo tên, công ty, email, số điện thoại...')).toBeTruthy();

    const checkboxes = screen.getAllByRole('checkbox');
    await fireEvent.click(checkboxes[0]);

    // Verify Vietnamese selection banner
    expect(screen.getByText('Bỏ chọn')).toBeTruthy();
    expect(screen.getByText(/Chọn tất cả 25/i)).toBeTruthy();
  });

  test('renders Vietnamese translated title for contacts mode when locale is vi', async () => {
    setLocale('vi');
    render(LeadsList, { props: { leads: mockLeads.slice(0, 2), entityType: 'contacts' } });

    expect(screen.getByText('Danh bạ Khách hàng đã Chuyển đổi')).toBeTruthy();
    expect(screen.getByText('Thêm Liên Hệ')).toBeTruthy();
  });

  test('resets or clamps currentPage when navigating pages and changing pageSize', async () => {
    render(LeadsList, { props: { leads: mockLeads } });

    // Initial page 1 shows Lead 1 to Lead 10
    expect(screen.getByText('Lead 1')).toBeTruthy();
    expect(screen.getByText('Showing 1 to 10 of 25')).toBeTruthy();

    // Find next page button (chevron right)
    const buttons = screen.getAllByRole('button');
    // Last button is next page
    const nextBtn = buttons[buttons.length - 1];
    await fireEvent.click(nextBtn); // Go to Page 2
    expect(screen.getByText('Showing 11 to 20 of 25')).toBeTruthy();

    await fireEvent.click(nextBtn); // Go to Page 3
    expect(screen.getByText('Showing 21 to 25 of 25')).toBeTruthy();
    expect(screen.getByText('Lead 25')).toBeTruthy();

    // Now change search to filter only 1 lead, which drops totalPages from 3 to 1
    const searchInput = screen.getByPlaceholderText('Search name, company, email, owner...');
    await fireEvent.input(searchInput, { target: { value: 'Lead 2' } });

    // currentPage must be reset to 1 and not remain 3 (which would cause empty table)
    expect(screen.getByText('Lead 2')).toBeTruthy();
    expect(screen.getByText(/Showing 1 to/i)).toBeTruthy();
  });

  test('filters leads by canonical VND value boundaries (9,999,999 / 10,000,000 / 50,000,000 / 50,000,001) and handles USD leads with FX conversion', () => {
    const boundaryLeads: Lead[] = [
      { id: 1, name: 'Lead 9.99M', company: 'Corp A', status: 'New', value: 9999999, currency: 'VND', last_activity: '2026-08-01', owner: 'Self', email: 'a@test.com', phone: '111' },
      { id: 2, name: 'Lead 10M', company: 'Corp B', status: 'New', value: 10000000, currency: 'VND', last_activity: '2026-08-01', owner: 'Self', email: 'b@test.com', phone: '222' },
      { id: 3, name: 'Lead 50M', company: 'Corp C', status: 'New', value: 50000000, currency: 'VND', last_activity: '2026-08-01', owner: 'Self', email: 'c@test.com', phone: '333' },
      { id: 4, name: 'Lead 50.001M', company: 'Corp D', status: 'New', value: 50000001, currency: 'VND', last_activity: '2026-08-01', owner: 'Self', email: 'd@test.com', phone: '444' },
      { id: 5, name: 'Lead USD 5K', company: 'Corp E', status: 'New', value: 5000, currency: 'USD', last_activity: '2026-08-01', owner: 'Self', email: 'e@test.com', phone: '555' },
      { id: 6, name: 'Lead USD 200', company: 'Corp F', status: 'New', value: 200, currency: 'USD', last_activity: '2026-08-01', owner: 'Self', email: 'f@test.com', phone: '666' }
    ];

    const filterFn = (leads: Lead[], range: string, rate = 25400) => {
      return leads.filter((lead) => {
        const valVnd = convertCurrency(lead.value || 0, (lead.currency as Currency) || 'VND', 'VND', rate);
        if (range === 'under_10m' && valVnd >= 10000000) return false;
        if (range === '10m_to_50m' && (valVnd < 10000000 || valVnd > 50000000)) return false;
        if (range === 'above_50m' && valVnd <= 50000000) return false;
        return true;
      });
    };

    // Under 10M: should match 9,999,999 VND and $200 USD (5,080,000 VND)
    const under10M = filterFn(boundaryLeads, 'under_10m');
    expect(under10M.map(l => l.name)).toEqual(['Lead 9.99M', 'Lead USD 200']);

    // 10M - 50M: should match 10,000,000 and 50,000,000 inclusive
    const between10M50M = filterFn(boundaryLeads, '10m_to_50m');
    expect(between10M50M.map(l => l.name)).toEqual(['Lead 10M', 'Lead 50M']);

    // Above 50M: should match 50,000,001 VND and $5,000 USD (127,000,000 VND)
    const above50M = filterFn(boundaryLeads, 'above_50m');
    expect(above50M.map(l => l.name)).toEqual(['Lead 50.001M', 'Lead USD 5K']);
  });
});
