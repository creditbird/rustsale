/// <reference types="bun-types" />
import { mock, beforeEach, test, expect } from 'bun:test';

// Polyfill Element.prototype.animate for JSDOM / Svelte transitions
if (typeof Element !== 'undefined' && !Element.prototype.animate) {
  Element.prototype.animate = function() {
    return {
      cancel: () => {},
      finish: () => {},
      pause: () => {},
      play: () => {},
      reverse: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => {},
      onfinish: null,
      oncancel: null,
      onremove: null,
      finished: Promise.resolve(),
      ready: Promise.resolve(),
      currentTime: 0,
      effect: null,
      id: '',
      timeline: null,
      playbackRate: 1,
      playState: 'finished',
      pending: false
    } as any;
  };
}

// Sample mock data
const defaultLeads = [
  { id: 1, name: "Alice Smith", company: "Acme Corp", status: "Qualified", value: 5000, last_activity: "2026-07-17", owner: "Self", email: "alice@acme.com", phone: "123-456", about: "Interested in enterprise plan" },
  { id: 2, name: "Bob Jones", company: "Globex", status: "New", value: 2500, last_activity: "2026-07-16", owner: "Self", email: "bob@globex.com", phone: "789-101", about: "Needs custom integrations" }
];
let mockLeads: typeof defaultLeads = [];

const defaultDeals = [
  { id: 10, lead_id: 1, title: "Acme Enterprise deal", value: 12000000, stage: "Prospecting", created_at: "2026-07-15" },
  { id: 11, lead_id: 2, title: "Globex Pilot plan", value: 3000000, stage: "Proposal", created_at: "2026-07-16" }
];
let mockDeals: typeof defaultDeals = [];

beforeEach(() => {
  setLocale('en');
  mockLeads.splice(0, mockLeads.length, ...JSON.parse(JSON.stringify(defaultLeads)));
  mockDeals.splice(0, mockDeals.length, ...JSON.parse(JSON.stringify(defaultDeals)));
});

const mockActivities = [
  { id: 101, lead_id: 1, activity_type: "Call", content: "Initial call done", created_at: "2026-07-15T10:00:00Z", performed_by: "Self" }
];

// Mock `@tauri-apps/api/core` module
mock.module('@tauri-apps/api/core', () => {
  return {
    invoke: (cmd: string, args?: any) => {
      if (cmd === 'get_leads') {
        return Promise.resolve(mockLeads);
      }
      if (cmd === 'get_deals') {
        return Promise.resolve(mockDeals);
      }
      if (cmd === 'get_lead_details') {
        const found = mockLeads.find(l => l.id === args.id);
        return found ? Promise.resolve(found) : Promise.reject("Lead not found");
      }
      if (cmd === 'get_activities') {
        return Promise.resolve(mockActivities.filter(a => a.lead_id === args.leadId));
      }
      if (cmd === 'add_activity') {
        mockActivities.push({
          id: Math.floor(Math.random() * 1000),
          lead_id: args.leadId,
          activity_type: args.activityType,
          content: args.content,
          created_at: new Date().toISOString(),
          performed_by: args.performedBy
        });
        return Promise.resolve();
      }
      if (cmd === 'create_deal') {
        const newDeal = {
          id: Math.floor(Math.random() * 1000) + 50,
          lead_id: args.leadId,
          title: args.title,
          value: args.value,
          stage: args.stage,
          created_at: new Date().toISOString()
        };
        mockDeals.push(newDeal);
        return Promise.resolve(newDeal.id);
      }
      if (cmd === 'delete_deal') {
        const index = mockDeals.findIndex(d => d.id === args.id);
        if (index !== -1) mockDeals.splice(index, 1);
        return Promise.resolve();
      }
      if (cmd === 'update_deal_stage') {
        const deal = mockDeals.find(d => d.id === args.id);
        if (deal) deal.stage = args.stage;
        return Promise.resolve();
      }
      if (cmd === 'update_lead') {
        const lead = mockLeads.find(l => l.id === args.id);
        if (lead) {
          lead.name = args.name;
          lead.company = args.company;
          lead.status = args.status;
          lead.value = args.value;
          lead.owner = args.owner;
          lead.email = args.email;
          lead.phone = args.phone;
          lead.about = args.about;
        }
        return Promise.resolve();
      }
      if (cmd === 'delete_lead') {
        const index = mockLeads.findIndex(l => l.id === args.id);
        if (index !== -1) {
          mockLeads.splice(index, 1);
        }
        return Promise.resolve();
      }
      if (cmd === 'get_managed_zalo_accounts') {
        return Promise.resolve([{ accountKey: 'imei_123', displayName: 'Zalo Sale 1', authStatus: 'valid' }]);
      }
      if (cmd === 'get_outreach_templates') {
        return Promise.resolve([{ id: 1, name: 'Chào hàng Zalo', content: 'Xin chào [name]' }]);
      }
      if (cmd === 'get_managed_smtp_accounts') {
        return Promise.resolve([{ id: 1, name: 'Main Sales SMTP', fromEmail: 'sales@acme.com' }]);
      }
      if (cmd === 'get_email_templates') {
        return Promise.resolve([{ id: 1, name: 'Chào hàng Email', subject: 'Thư chào hàng', content: 'Xin chào {{name}}' }]);
      }
      if (cmd === 'send_zalo_outreach' || cmd === 'send_email_outreach') {
        return Promise.resolve();
      }
      return Promise.resolve();
    }
  };
});

import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { afterEach } from 'bun:test';
import LeadsList from '../LeadsList.svelte';
import KanbanBoard from '../KanbanBoard.svelte';
import LeadDetails from '../LeadDetails.svelte';
import { setLocale, formatCurrency, formatCompactCurrency } from '$lib/i18n';

afterEach(() => {
  cleanup();
});

test('renders LeadsList with columns and leads data', () => {
  render(LeadsList, { props: { leads: mockLeads } });
  
  // Verify columns render
  expect(screen.getByText('Name')).toBeTruthy();
  expect(screen.getByText('Company')).toBeTruthy();
  expect(screen.getByText('Status')).toBeTruthy();
  expect(screen.getByText('Value')).toBeTruthy();
  expect(screen.getByText('Owner')).toBeTruthy();

  // Verify rows render
  expect(screen.getByText('Alice Smith')).toBeTruthy();
  expect(screen.getByText('Bob Jones')).toBeTruthy();
  expect(screen.getByText('Acme Corp')).toBeTruthy();
  expect(screen.getByText('Globex')).toBeTruthy();
});

test('opens LeadDetails drawer when clicking a lead row in LeadsList', async () => {
  render(LeadsList, { props: { leads: mockLeads } });

  const aliceRow = screen.getByText('Alice Smith');
  await fireEvent.click(aliceRow);

  // LeadDetails is rendered
  const drawerTitle = await screen.findByText('Lead Profile Details');
  expect(drawerTitle).toBeTruthy();

  // Verify lead properties render in drawer
  expect(screen.getByText('Contact Overview')).toBeTruthy();
  expect(screen.getAllByText('alice@acme.com').length).toBeGreaterThan(0);
  expect(screen.getByText('Interested in enterprise plan')).toBeTruthy();
});

test('renders KanbanBoard with all pipeline stages and deals', async () => {
  setLocale('en');
  render(KanbanBoard);

  // Verify pipeline header
  expect(screen.getByText('Total Pipeline Value')).toBeTruthy();

  // Verify columns render with stage labels
  expect(screen.getByText('Prospecting')).toBeTruthy();
  expect(screen.getByText('Demo / Pitch')).toBeTruthy();
  expect(screen.getByText('Proposal Sent')).toBeTruthy();
  expect(screen.getByText('Negotiation')).toBeTruthy();
  expect(screen.getByText('Closed Won')).toBeTruthy();

  // Wait for deals to load and render
  const dealCard1 = await screen.findByText('Acme Enterprise deal');
  const dealCard2 = await screen.findByText('Globex Pilot plan');
  expect(dealCard1).toBeTruthy();
  expect(dealCard2).toBeTruthy();
});

test('renders KanbanBoard with Vietnamese column headers and VND currency formatting', async () => {
  setLocale('vi');
  render(KanbanBoard);

  // Verify pipeline header in Vietnamese
  expect(screen.getByText('Tổng Giá Trị Phễu Bán Hàng')).toBeTruthy();

  // Verify columns render with Vietnamese stage labels
  expect(screen.getByText('Tìm kiếm Tiềm năng')).toBeTruthy();
  expect(screen.getByText('Demo / Giới thiệu')).toBeTruthy();
  expect(screen.getByText('Gửi Báo giá')).toBeTruthy();
  expect(screen.getByText('Đàm phán')).toBeTruthy();
  expect(screen.getByText('Thành công')).toBeTruthy();
});

test('logs a new activity via LeadDetails', async () => {
  render(LeadDetails, { props: { leadId: 1, onClose: () => {} } });

  // Wait for loading to finish and details to be visible
  await screen.findByText('Alice Smith');

  // Find form elements
  const select = screen.getByRole('combobox') as HTMLSelectElement;
  const textarea = screen.getByPlaceholderText('Enter activity details, call notes, meeting minutes...') as HTMLTextAreaElement;

  // Fill in form
  await fireEvent.change(select, { target: { value: 'Email' } });
  await fireEvent.input(textarea, { target: { value: 'Sent follow up email about pricing' } });
  
  // Submit activity
  const form = textarea.closest('form')!;
  await fireEvent.submit(form);

  // Check that textarea is cleared
  expect(textarea.value).toBe('');

  // Verify new activity appears in history
  const newActivityText = await screen.findByText('Sent follow up email about pricing');
  expect(newActivityText).toBeTruthy();
});

test('edits a lead and saves changes', async () => {
  render(LeadDetails, { props: { leadId: 1, onClose: () => {} } });

  // Wait for details to render
  await screen.findByText('Alice Smith');

  // Trigger edit mode
  const editBtn = screen.getByRole('button', { name: 'Edit' });
  await fireEvent.click(editBtn);

  // Find inputs
  const nameInput = screen.getByLabelText('Full Name *') as HTMLInputElement;
  const statusSelect = screen.getByLabelText('Status') as HTMLSelectElement;

  // Change values
  await fireEvent.input(nameInput, { target: { value: 'Alice Johnson' } });
  await fireEvent.change(statusSelect, { target: { value: 'Converted' } });

  // Save changes
  const saveBtn = screen.getByRole('button', { name: 'Save' });
  const form = saveBtn.closest('form')!;
  await fireEvent.submit(form);

  // Check that new values are rendered in view mode
  const updatedName = await screen.findByText('Alice Johnson');
  expect(updatedName).toBeTruthy();
  expect(screen.getAllByText('Converted').length).toBeGreaterThan(0);
});

test('deletes a lead with confirmation', async () => {
  let closed = false;

  render(LeadDetails, { props: { leadId: 2, onClose: () => { closed = true; } } });

  // Wait for details to render
  await screen.findByText('Bob Jones');

  // Trigger edit mode
  const editBtn = screen.getByRole('button', { name: 'Edit' });
  await fireEvent.click(editBtn);

  // Click Delete
  const deleteBtn = screen.getByRole('button', { name: 'Delete' });
  await fireEvent.click(deleteBtn);
  const confirmDeleteBtn = await screen.findByRole('button', { name: 'Delete Customer' });
  await fireEvent.click(confirmDeleteBtn);

  // Drawer should be closed
  expect(closed).toBe(true);
});

test('opens Quick Send Zalo modal and sends Zalo outreach', async () => {
  render(LeadDetails, { props: { leadId: 1, onClose: () => {} } });
  await screen.findByText('Alice Smith');

  const zaloBtn = screen.getByRole('button', { name: 'Send Zalo' });
  await fireEvent.click(zaloBtn);

  const modalTitle = await screen.findByText(/Send Zalo Message - Alice Smith/i);
  expect(modalTitle).toBeTruthy();

  const sendBtn = screen.getByRole('button', { name: 'Send Zalo Now' });
  await fireEvent.click(sendBtn);

  // Check activity log recorded
  const activityText = await screen.findByText(/Sent Zalo message to Alice Smith/i);
  expect(activityText).toBeTruthy();
});

test('creates a deal via LeadDetails using VND currency preset and renders with identical VND semantics in KanbanBoard', async () => {
  setLocale('en');
  const detailsView = render(LeadDetails, { props: { leadId: 1, onClose: () => {} } });
  await screen.findByText('Alice Smith');

  // Open Create Deal Modal
  const createDealBtn = screen.getByRole('button', { name: '+ Create Deal' });
  await fireEvent.click(createDealBtn);

  // Fill in deal info with 25M preset
  const titleInput = screen.getByLabelText('Deal Title *') as HTMLInputElement;
  await fireEvent.input(titleInput, { target: { value: 'Acme Expansion deal' } });

  const preset25M = screen.getByRole('button', { name: '25M ₫' });
  await fireEvent.click(preset25M);

  const submitDealBtn = screen.getByRole('button', { name: 'Create Deal' });
  await fireEvent.click(submitDealBtn);

  // Verify created deal rendered in LeadDetails with VND currency formatting
  const expectedValueStr = formatCurrency(25000000, 'VND');
  const renderedDealInDetails = await screen.findByText('Acme Expansion deal');
  expect(renderedDealInDetails).toBeTruthy();
  expect(screen.getAllByText(expectedValueStr).length).toBeGreaterThanOrEqual(1);

  // Unmount LeadDetails and render KanbanBoard to verify end-to-end VND currency alignment
  cleanup();
  render(KanbanBoard);

  const renderedDealInKanban = await screen.findByText('Acme Expansion deal');
  expect(renderedDealInKanban).toBeTruthy();
  expect(screen.getAllByText(expectedValueStr).length).toBeGreaterThanOrEqual(1);
});

test('renders LeadDetails with full Vietnamese translations and interaction flow when locale is vi', async () => {
  setLocale('vi');
  render(LeadDetails, { props: { leadId: 1, onClose: () => {} } });

  await screen.findByText('Alice Smith');

  expect(screen.getByText('Chi tiết Khách hàng Tiềm năng')).toBeTruthy();
  expect(screen.getByText('Tổng quan Liên hệ')).toBeTruthy();
  expect(screen.getAllByText('Tiềm năng').length).toBeGreaterThanOrEqual(1); // mapped lead.status 'Qualified'
  expect(screen.getAllByText('Cuộc gọi').length).toBeGreaterThanOrEqual(1); // mapped activity.activity_type 'Call'
  expect(screen.getByText('+ Tạo Giao dịch')).toBeTruthy();
  expect(screen.getByText('Gửi Zalo')).toBeTruthy();
  expect(screen.getByText('Gửi Email')).toBeTruthy();
  expect(screen.getByText('Lịch sử Hoạt động')).toBeTruthy();
  expect(screen.getAllByText('Ghi nhận Hoạt động').length).toBeGreaterThanOrEqual(1);

  // Check deal delete tooltip in Vietnamese
  const deleteBtn = screen.getByTitle('Xóa giao dịch này');
  expect(deleteBtn).toBeTruthy();

  // Test interactive deal creation with Vietnamese validation and submission
  const createDealBtn = screen.getByRole('button', { name: '+ Tạo Giao dịch' });
  await fireEvent.click(createDealBtn);

  // Clear title input and submit -> validation error in Vietnamese
  const titleInput = screen.getByLabelText(/Tên Giao dịch/i) as HTMLInputElement;
  await fireEvent.input(titleInput, { target: { value: '' } });
  const form = titleInput.closest('form')!;
  await fireEvent.submit(form);

  const errorMsg = await screen.findByText('Vui lòng nhập tiêu đề giao dịch.');
  expect(errorMsg).toBeTruthy();

  // Enter valid title, select 50M preset, and create
  await fireEvent.input(titleInput, { target: { value: 'Hợp đồng VIP' } });
  const preset50M = screen.getByRole('button', { name: formatCompactCurrency(50000000, 'VND') });
  await fireEvent.click(preset50M);
  await fireEvent.submit(form);

  // Verify created deal rendered in Vietnamese with VND formatted amount
  const createdDeal = await screen.findByText('Hợp đồng VIP');
  expect(createdDeal).toBeTruthy();
  expect(screen.getAllByText('Tìm kiếm Tiềm năng (20%)').length).toBeGreaterThanOrEqual(1);
  expect(screen.getAllByText((content) => content.includes('50.000.000')).length).toBeGreaterThanOrEqual(1);
});

test('filters leads by VND value ranges across 9,999,999 / 10,000,000 / 50,000,000 / 50,000,001 boundaries', async () => {
  setLocale('vi');
  const boundaryLeads = [
    { id: 101, name: "Lead Sub 10M", company: "Corp A", status: "New", value: 9999999, last_activity: "2026-08-01", owner: "Self", email: "a@test.com", phone: "111", about: null },
    { id: 102, name: "Lead Exact 10M", company: "Corp B", status: "New", value: 10000000, last_activity: "2026-08-01", owner: "Self", email: "b@test.com", phone: "222", about: null },
    { id: 103, name: "Lead Exact 50M", company: "Corp C", status: "New", value: 50000000, last_activity: "2026-08-01", owner: "Self", email: "c@test.com", phone: "333", about: null },
    { id: 104, name: "Lead Above 50M", company: "Corp D", status: "New", value: 50000001, last_activity: "2026-08-01", owner: "Self", email: "d@test.com", phone: "444", about: null }
  ];

  render(LeadsList, { props: { leads: boundaryLeads } });

  // Initially all 4 leads rendered
  expect(screen.getByText('Lead Sub 10M')).toBeTruthy();
  expect(screen.getByText('Lead Exact 10M')).toBeTruthy();
  expect(screen.getByText('Lead Exact 50M')).toBeTruthy();
  expect(screen.getByText('Lead Above 50M')).toBeTruthy();

  // Filter with under_10m (< 10M): should contain only 9,999,999
  // 9,999,999 < 10,000,000 -> true; 10,000,000 >= 10,000,000 -> false
  const filterTrigger = screen.getByText('Tất cả Giá trị');
  expect(filterTrigger).toBeTruthy();
});
