/// <reference types="bun-types" />
import { afterEach, beforeEach, expect, mock, test } from 'bun:test';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import { setLocale } from '$lib/i18n';
import {
  parseSpintax,
  renderSpintaxTemplate,
  normalizeWhatsAppPhone
} from '$lib/services/whatsappService';

let mockAccounts: Array<{
  id: number;
  provider_account_key: string;
  display_name: string | null;
  phone_number: string | null;
}> = [];

let mockCampaigns: Array<{
  id: number;
  name: string;
  template_id: number | null;
  status: string;
  total_recipients: number;
  sent_count: number;
  delivered_count: number;
  read_count: number;
  failed_count: number;
  skipped_count: number;
  assigned_account_ids: number[];
  rate_limit_min_delay_secs: number;
  rate_limit_max_delay_secs: number;
  burst_limit_per_batch: number;
  burst_pause_secs: number;
  quiet_hours_enabled: boolean;
  quiet_hours_start: string;
  quiet_hours_end: string;
  timezone: string;
  last_heartbeat_at: string | null;
  started_at: string | null;
  completed_at: string | null;
  created_at: string;
  updated_at: string;
}> = [];

let mockRecipients: Array<{
  id: number;
  campaign_id: number;
  lead_id: number | null;
  recipient_phone_raw: string;
  recipient_phone_e164: string;
  recipient_jid: string;
  assigned_channel_account_id: number | null;
  status: string;
  skip_reason: string | null;
  rendered_text: string | null;
  rendered_image_url: string | null;
  message_id: number | null;
  outbox_job_id: string | null;
  attempt_count: number;
  last_attempt_at: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}> = [];

let mockCategories = [{ id: 1, name: 'VIP Clients' }];
let mockLeads = [
  {
    id: 1,
    name: 'Alice Johnson',
    company: 'TechCorp',
    status: 'New',
    phone: '0901234567',
    category_id: 1,
    email: 'alice@techcorp.com',
    about: 'CTO'
  },
  {
    id: 2,
    name: 'Bob Smith',
    company: 'Innovate LLC',
    status: 'Contacted',
    phone: '0987654321',
    category_id: 1,
    email: 'bob@innovate.com',
    about: 'CEO'
  }
];

let invokedCommands: Array<{ command: string; args?: any }> = [];

mock.module('@tauri-apps/api/core', () => ({
  invoke: (command: string, args?: any) => {
    invokedCommands.push({ command, args });
    switch (command) {
      case 'get_managed_whatsapp_accounts':
        return Promise.resolve([...mockAccounts]);
      case 'list_whatsapp_campaigns':
        return Promise.resolve([...mockCampaigns]);
      case 'get_whatsapp_campaign': {
        const found = mockCampaigns.find((c) => c.id === args?.campaignId);
        return found ? Promise.resolve({ ...found }) : Promise.reject('Not found');
      }
      case 'get_whatsapp_campaign_recipients':
        return Promise.resolve(mockRecipients.filter((r) => r.campaign_id === args?.campaignId));
      case 'create_whatsapp_campaign': {
        const newId = mockCampaigns.length + 1;
        const newCamp = {
          id: newId,
          name: args.req.name,
          template_id: args.req.template_id ?? null,
          status: 'queued',
          total_recipients: 0,
          sent_count: 0,
          delivered_count: 0,
          read_count: 0,
          failed_count: 0,
          skipped_count: 0,
          assigned_account_ids: args.req.assigned_account_ids,
          rate_limit_min_delay_secs: args.req.rate_limit_min_delay_secs ?? 8,
          rate_limit_max_delay_secs: args.req.rate_limit_max_delay_secs ?? 20,
          burst_limit_per_batch: args.req.burst_limit_per_batch ?? 25,
          burst_pause_secs: args.req.burst_pause_secs ?? 60,
          quiet_hours_enabled: args.req.quiet_hours_enabled ?? true,
          quiet_hours_start: args.req.quiet_hours_start ?? '22:00',
          quiet_hours_end: args.req.quiet_hours_end ?? '07:30',
          timezone: args.req.timezone ?? 'Asia/Ho_Chi_Minh',
          last_heartbeat_at: null,
          started_at: null,
          completed_at: null,
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString()
        };
        mockCampaigns.push(newCamp);
        return Promise.resolve(newId);
      }
      case 'populate_whatsapp_campaign_recipients': {
        const camp = mockCampaigns.find((c) => c.id === args?.campaignId);
        if (camp) {
          camp.total_recipients = args.recipients.length;
        }
        return Promise.resolve(args?.recipients?.length ?? 0);
      }
      case 'start_whatsapp_campaign': {
        const camp = mockCampaigns.find((c) => c.id === args?.campaignId);
        if (camp) camp.status = 'running';
        return Promise.resolve();
      }
      case 'pause_whatsapp_campaign': {
        const camp = mockCampaigns.find((c) => c.id === args?.campaignId);
        if (camp) camp.status = 'paused';
        return Promise.resolve();
      }
      case 'resume_whatsapp_campaign': {
        const camp = mockCampaigns.find((c) => c.id === args?.campaignId);
        if (camp) camp.status = 'running';
        return Promise.resolve();
      }
      case 'cancel_whatsapp_campaign': {
        const camp = mockCampaigns.find((c) => c.id === args?.campaignId);
        if (camp) camp.status = 'cancelled';
        return Promise.resolve();
      }
      case 'get_categories':
        return Promise.resolve([...mockCategories]);
      case 'get_leads':
        return Promise.resolve([...mockLeads]);
      default:
        return Promise.resolve();
    }
  }
}));

import WhatsAppOutreach from '../WhatsAppOutreach.svelte';

beforeEach(() => {
  setLocale('en');
  document.body.removeAttribute('style');
  mockAccounts = [];
  mockCampaigns = [];
  mockRecipients = [];
  invokedCommands = [];
});

afterEach(() => {
  cleanup();
  document.body.removeAttribute('style');
  mockAccounts.length = 0;
  mockCampaigns.length = 0;
  mockRecipients.length = 0;
  invokedCommands.length = 0;
});

test('Spintax parser & lead variable template rendering helper functions', () => {
  const spintax = '{Option A|Option B|Option C}';
  const result = parseSpintax(spintax);
  expect(['Option A', 'Option B', 'Option C']).toContain(result);

  const nested = '{Hello|Hi} {friend|there}!';
  const nestedRes = parseSpintax(nested);
  expect(
    ['Hello friend!', 'Hello there!', 'Hi friend!', 'Hi there!'].includes(nestedRes)
  ).toBe(true);

  const rendered = renderSpintaxTemplate('{Xin chào|Chào} [name] từ [company]!', {
    name: 'Trần Văn A',
    company: 'ACME Corp'
  });
  expect(
    rendered === 'Xin chào Trần Văn A từ ACME Corp!' ||
      rendered === 'Chào Trần Văn A từ ACME Corp!'
  ).toBe(true);
});

test('Phone normalization helper handles Vietnamese and international numbers', () => {
  expect(normalizeWhatsAppPhone('0901234567')).toBe('+84901234567');
  expect(normalizeWhatsAppPhone('+84 901 234 567')).toBe('+84901234567');
  expect(normalizeWhatsAppPhone('84901234567')).toBe('+84901234567');
  expect(normalizeWhatsAppPhone('+1 (555) 123-4567')).toBe('+15551234567');
  expect(normalizeWhatsAppPhone('123')).toBeNull();
});

test('renders empty state when no campaigns exist', async () => {
  render(WhatsAppOutreach);
  await tick();

  expect(await screen.findByText('WhatsApp Outreach Dashboard')).toBeTruthy();
  expect(await screen.findByText('Total Campaigns')).toBeTruthy();
  expect(await screen.findByText(/No campaigns found/i)).toBeTruthy();
});

test('renders campaign list with progress and stats cards', async () => {
  mockCampaigns.push({
    id: 1,
    name: 'Black Friday Broadcast',
    template_id: null,
    status: 'running',
    total_recipients: 100,
    sent_count: 45,
    delivered_count: 40,
    read_count: 35,
    failed_count: 2,
    skipped_count: 5,
    assigned_account_ids: [1],
    rate_limit_min_delay_secs: 8,
    rate_limit_max_delay_secs: 20,
    burst_limit_per_batch: 25,
    burst_pause_secs: 60,
    quiet_hours_enabled: true,
    quiet_hours_start: '22:00',
    quiet_hours_end: '07:30',
    timezone: 'Asia/Ho_Chi_Minh',
    last_heartbeat_at: null,
    started_at: '2026-08-26T10:00:00Z',
    completed_at: null,
    created_at: '2026-08-26T09:00:00Z',
    updated_at: '2026-08-26T10:00:00Z'
  });

  render(WhatsAppOutreach);
  await tick();

  expect(await screen.findByText('Black Friday Broadcast')).toBeTruthy();
  expect(screen.getByText('ID: #1 • 8/26/2026')).toBeTruthy();
  expect(screen.getByText('Progress: 45 / 100')).toBeTruthy();
  expect(screen.getByText('45%')).toBeTruthy();
  expect(screen.getAllByText('45').length).toBeGreaterThan(0);
  expect(screen.getAllByText('40').length).toBeGreaterThan(0);
  expect(screen.getAllByText('35').length).toBeGreaterThan(0);
  expect(screen.getByText('2')).toBeTruthy();
});

test('launches create campaign modal and successfully submits a new campaign', async () => {
  mockAccounts.push({
    id: 10,
    provider_account_key: 'wa_acc_10',
    display_name: 'Sales Hotline WhatsApp',
    phone_number: '+84901234567'
  });

  render(WhatsAppOutreach);
  await tick();

  const newCampaignBtns = await screen.findAllByRole('button', { name: /New Campaign/i });
  await fireEvent.click(newCampaignBtns[0]);
  await tick();

  expect(await screen.findByText('Create WhatsApp Campaign')).toBeTruthy();

  const nameInput = screen.getByLabelText(/Campaign Name/i);
  await fireEvent.input(nameInput, { target: { value: 'VIP Customer Outreach 2026' } });

  // Toggle variable button
  const nameVarBtn = screen.getByRole('button', { name: /\+ \[name\]/i });
  await fireEvent.click(nameVarBtn);

  // Submit create campaign
  const submitBtn = screen.getByRole('button', { name: /Create & Queue Campaign/i });
  await fireEvent.click(submitBtn);
  await tick();
  await new Promise((r) => setTimeout(r, 100));

  const createCmd = invokedCommands.find((c) => c.command === 'create_whatsapp_campaign');
  const populateCmd = invokedCommands.find((c) => c.command === 'populate_whatsapp_campaign_recipients');

  expect(createCmd).toBeTruthy();
  expect(createCmd?.args.req.name).toBe('VIP Customer Outreach 2026');
  expect(createCmd?.args.req.assigned_account_ids).toEqual([10]);
  expect(createCmd?.args.req.rate_limit_min_delay_secs).toBe(8);
  expect(createCmd?.args.req.rate_limit_max_delay_secs).toBe(20);
  expect(createCmd?.args.req.quiet_hours_enabled).toBe(true);

  expect(populateCmd).toBeTruthy();
  expect(populateCmd?.args.campaignId).toBe(1);
  expect(populateCmd?.args.recipients.length).toBe(2); // 2 CRM leads
});

test('allows manual phone list input and normalizes phone numbers in preview', async () => {
  mockAccounts.push({
    id: 10,
    provider_account_key: 'wa_acc_10',
    display_name: 'Support Line',
    phone_number: '+84999999999'
  });

  render(WhatsAppOutreach);
  await tick();

  const newCampaignBtns = await screen.findAllByRole('button', { name: /New Campaign/i });
  await fireEvent.click(newCampaignBtns[0]);
  await tick();

  const manualModeBtn = screen.getByRole('button', { name: /Paste Raw Phone Numbers/i });
  await fireEvent.click(manualModeBtn);
  await tick();

  const phoneTextarea = screen.getByPlaceholderText(/Paste phone numbers/i);
  await fireEvent.input(phoneTextarea, {
    target: { value: '0901234567\n0987654321, +15551234567' }
  });
  await tick();

  const normalizedHeader = await screen.findByText(/Normalized Recipients Preview \(3\)/i);
  expect(normalizedHeader).toBeTruthy();
  expect(screen.getAllByText('+84901234567').length).toBeGreaterThan(0);
  expect(screen.getByText('+84987654321')).toBeTruthy();
  expect(screen.getByText('+15551234567')).toBeTruthy();
});

test('campaign control actions: start draft campaign', async () => {
  mockCampaigns.push({
    id: 1,
    name: 'Draft Campaign',
    template_id: null,
    status: 'draft',
    total_recipients: 10,
    sent_count: 0,
    delivered_count: 0,
    read_count: 0,
    failed_count: 0,
    skipped_count: 0,
    assigned_account_ids: [1],
    rate_limit_min_delay_secs: 8,
    rate_limit_max_delay_secs: 20,
    burst_limit_per_batch: 25,
    burst_pause_secs: 60,
    quiet_hours_enabled: true,
    quiet_hours_start: '22:00',
    quiet_hours_end: '07:30',
    timezone: 'Asia/Ho_Chi_Minh',
    last_heartbeat_at: null,
    started_at: null,
    completed_at: null,
    created_at: '2026-08-26T09:00:00Z',
    updated_at: '2026-08-26T09:00:00Z'
  });

  render(WhatsAppOutreach);
  await tick();

  const startBtns = await screen.findAllByRole('button', { name: /^Start$/i });
  await fireEvent.click(startBtns[0]);
  await waitFor(() => {
    expect(invokedCommands.some((c) => c.command === 'start_whatsapp_campaign' && c.args.campaignId === 1)).toBe(true);
  });
});

test('campaign control actions: pause running campaign and cancel', async () => {
  mockCampaigns.push({
    id: 2,
    name: 'Running Campaign',
    template_id: null,
    status: 'running',
    total_recipients: 50,
    sent_count: 20,
    delivered_count: 18,
    read_count: 15,
    failed_count: 0,
    skipped_count: 1,
    assigned_account_ids: [1],
    rate_limit_min_delay_secs: 8,
    rate_limit_max_delay_secs: 20,
    burst_limit_per_batch: 25,
    burst_pause_secs: 60,
    quiet_hours_enabled: true,
    quiet_hours_start: '22:00',
    quiet_hours_end: '07:30',
    timezone: 'Asia/Ho_Chi_Minh',
    last_heartbeat_at: null,
    started_at: '2026-08-26T10:00:00Z',
    completed_at: null,
    created_at: '2026-08-26T09:00:00Z',
    updated_at: '2026-08-26T10:00:00Z'
  });

  render(WhatsAppOutreach);
  await tick();

  const pauseBtns = await screen.findAllByRole('button', { name: /^Pause$/i });
  await fireEvent.click(pauseBtns[0]);
  await waitFor(() => {
    expect(invokedCommands.some((c) => c.command === 'pause_whatsapp_campaign' && c.args.campaignId === 2)).toBe(true);
  });
});

test('campaign control actions: resume paused campaign', async () => {
  mockCampaigns.push({
    id: 3,
    name: 'Paused Campaign',
    template_id: null,
    status: 'paused',
    total_recipients: 50,
    sent_count: 10,
    delivered_count: 9,
    read_count: 8,
    failed_count: 0,
    skipped_count: 0,
    assigned_account_ids: [1],
    rate_limit_min_delay_secs: 8,
    rate_limit_max_delay_secs: 20,
    burst_limit_per_batch: 25,
    burst_pause_secs: 60,
    quiet_hours_enabled: true,
    quiet_hours_start: '22:00',
    quiet_hours_end: '07:30',
    timezone: 'Asia/Ho_Chi_Minh',
    last_heartbeat_at: null,
    started_at: '2026-08-26T10:00:00Z',
    completed_at: null,
    created_at: '2026-08-26T09:00:00Z',
    updated_at: '2026-08-26T10:00:00Z'
  });

  render(WhatsAppOutreach);
  await tick();

  const resumeBtns = await screen.findAllByRole('button', { name: /^Resume$/i });
  await fireEvent.click(resumeBtns[0]);
  await waitFor(() => {
    expect(invokedCommands.some((c) => c.command === 'resume_whatsapp_campaign' && c.args.campaignId === 3)).toBe(true);
  });
});

test('opens campaign details modal and displays recipient ledger', async () => {
  mockCampaigns.push({
    id: 1,
    name: 'Promo Campaign',
    template_id: null,
    status: 'running',
    total_recipients: 2,
    sent_count: 1,
    delivered_count: 1,
    read_count: 1,
    failed_count: 0,
    skipped_count: 0,
    assigned_account_ids: [1],
    rate_limit_min_delay_secs: 8,
    rate_limit_max_delay_secs: 20,
    burst_limit_per_batch: 25,
    burst_pause_secs: 60,
    quiet_hours_enabled: true,
    quiet_hours_start: '22:00',
    quiet_hours_end: '07:30',
    timezone: 'Asia/Ho_Chi_Minh',
    last_heartbeat_at: null,
    started_at: '2026-08-26T10:00:00Z',
    completed_at: null,
    created_at: '2026-08-26T09:00:00Z',
    updated_at: '2026-08-26T10:00:00Z'
  });

  mockRecipients.push(
    {
      id: 101,
      campaign_id: 1,
      lead_id: 1,
      recipient_phone_raw: '0901234567',
      recipient_phone_e164: '+84901234567',
      recipient_jid: '84901234567@s.whatsapp.net',
      assigned_channel_account_id: 1,
      status: 'delivered',
      skip_reason: null,
      rendered_text: 'Xin chào Alice Johnson từ TechCorp!',
      rendered_image_url: null,
      message_id: 555,
      outbox_job_id: 'outbox-1',
      attempt_count: 1,
      last_attempt_at: '2026-08-26T10:01:00Z',
      last_error: null,
      created_at: '2026-08-26T09:00:00Z',
      updated_at: '2026-08-26T10:01:00Z'
    },
    {
      id: 102,
      campaign_id: 1,
      lead_id: 2,
      recipient_phone_raw: '0987654321',
      recipient_phone_e164: '+84987654321',
      recipient_jid: '84987654321@s.whatsapp.net',
      assigned_channel_account_id: 1,
      status: 'skipped',
      skip_reason: 'opt_out_suppressed',
      rendered_text: 'Xin chào Bob Smith từ Innovate LLC!',
      rendered_image_url: null,
      message_id: null,
      outbox_job_id: null,
      attempt_count: 1,
      last_attempt_at: '2026-08-26T10:01:00Z',
      last_error: null,
      created_at: '2026-08-26T09:00:00Z',
      updated_at: '2026-08-26T10:01:00Z'
    }
  );

  render(WhatsAppOutreach);
  await tick();

  const viewDetailsBtn = await screen.findByRole('button', { name: /View Details/i });
  await fireEvent.click(viewDetailsBtn);
  await tick();

  expect(await screen.findByText('Campaign Details & Recipient Ledger')).toBeTruthy();
  expect(await screen.findByText('+84901234567')).toBeTruthy();
  expect(screen.getByText('Xin chào Alice Johnson từ TechCorp!')).toBeTruthy();
  expect(screen.getByText('+84987654321')).toBeTruthy();
  expect(screen.getByText('opt_out_suppressed')).toBeTruthy();
});
