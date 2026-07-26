/// <reference types="bun-types" />
import { mock } from 'bun:test';

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
const mockLeads = [
  { id: 1, name: "Alice Smith", company: "Acme Corp", status: "Qualified", value: 5000, last_activity: "2026-07-17", owner: "Self", email: "alice@acme.com", phone: "123-456", about: "Interested in enterprise plan" },
  { id: 2, name: "Bob Jones", company: "Globex", status: "New", value: 2500, last_activity: "2026-07-16", owner: "Self", email: "bob@globex.com", phone: "789-101", about: "Needs custom integrations" }
];

const mockDeals = [
  { id: 10, lead_id: 1, title: "Acme Enterprise deal", value: 12000, stage: "Prospecting", created_at: "2026-07-15" },
  { id: 11, lead_id: 2, title: "Globex Pilot plan", value: 3000, stage: "Proposal", created_at: "2026-07-16" }
];

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
      return Promise.resolve();
    }
  };
});

import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import { expect, test, afterEach } from 'bun:test';
import LeadsList from '../LeadsList.svelte';
import KanbanBoard from '../KanbanBoard.svelte';
import LeadDetails from '../LeadDetails.svelte';

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
  const drawerTitle = await screen.findByText('Chi Tiết Hồ Sơ Khách Hàng');
  expect(drawerTitle).toBeTruthy();

  // Verify lead properties render in drawer
  expect(screen.getByText('Tổng Quan Liên Hệ')).toBeTruthy();
  expect(screen.getAllByText('alice@acme.com').length).toBeGreaterThan(0);
  expect(screen.getByText('Interested in enterprise plan')).toBeTruthy();
});

test('renders KanbanBoard with all pipeline stages and deals', async () => {
  render(KanbanBoard);

  // Verify pipeline header
  expect(screen.getByText('Tổng Giá Trị Pipeline')).toBeTruthy();

  // Verify columns render
  expect(screen.getByText('Prospecting')).toBeTruthy();
  expect(screen.getByText('Demo')).toBeTruthy();
  expect(screen.getByText('Proposal')).toBeTruthy();
  expect(screen.getByText('Negotiation')).toBeTruthy();
  expect(screen.getByText('Won')).toBeTruthy();

  // Wait for deals to load and render
  const dealCard1 = await screen.findByText('Acme Enterprise deal');
  const dealCard2 = await screen.findByText('Globex Pilot plan');
  expect(dealCard1).toBeTruthy();
  expect(dealCard2).toBeTruthy();
});

test('logs a new activity via LeadDetails', async () => {
  render(LeadDetails, { props: { leadId: 1, onClose: () => {} } });

  // Wait for loading to finish and details to be visible
  await screen.findByText('Alice Smith');

  // Find form elements
  const select = screen.getByRole('combobox') as HTMLSelectElement;
  const textarea = screen.getByPlaceholderText('Nhập nội dung trao đổi, ghi chú cuộc gọi/cuộc họp...') as HTMLTextAreaElement;

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
  const editBtn = screen.getByRole('button', { name: 'Sửa' });
  await fireEvent.click(editBtn);

  // Find inputs
  const nameInput = screen.getByLabelText('Họ và Tên *') as HTMLInputElement;
  const statusSelect = screen.getByLabelText('Trạng Thái') as HTMLSelectElement;

  // Change values
  await fireEvent.input(nameInput, { target: { value: 'Alice Johnson' } });
  await fireEvent.change(statusSelect, { target: { value: 'Converted' } });

  // Save changes
  const saveBtn = screen.getByRole('button', { name: 'Lưu Thay Đổi' });
  await fireEvent.click(saveBtn);

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
  const editBtn = screen.getByRole('button', { name: 'Sửa' });
  await fireEvent.click(editBtn);

  // Click Delete
  const deleteBtn = screen.getByRole('button', { name: 'Xóa Lead' });
  await fireEvent.click(deleteBtn);
  const confirmDeleteBtn = await screen.findByRole('button', { name: 'Xóa Khách Hàng' });
  await fireEvent.click(confirmDeleteBtn);

  // Drawer should be closed
  expect(closed).toBe(true);
});
