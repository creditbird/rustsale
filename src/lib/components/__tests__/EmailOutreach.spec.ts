/// <reference types="bun-types" />
import { afterEach, beforeEach, expect, mock, test } from 'bun:test';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';

type EventListener = (event: { payload: unknown }) => void;

const eventListeners = new Map<string, EventListener>();
let mockSmtpServers: Array<{
  id: number;
  name: string;
  host: string;
  port: number;
  username: string;
  password: string;
  from_email: string;
  from_name: string;
  security: string;
  created_at: string;
}> = [];

let mockEmailTemplates: Array<{
  id: number;
  name: string;
  subject: string;
  content: string;
  created_at: string;
}> = [];
let mockCategories = [{ id: 1, name: 'VIP Clients' }];
let smtpFailuresRemaining = 0;

const defaultLeads = [
  {
    id: 1,
    name: 'Alice Johnson',
    company: 'TechCorp',
    status: 'New',
    value: 5000,
    last_activity: '2026-07-20',
    owner: 'Sales',
    email: 'alice@techcorp.com',
    phone: '123456789',
    about: 'CTO',
    address: 'Hanoi',
    has_zalo: 1,
    zalo_reached: 0
  },
  {
    id: 2,
    name: 'Bob Smith',
    company: 'Innovate LLC',
    status: 'Contacted',
    value: 10000,
    last_activity: '2026-07-21',
    owner: 'Sales',
    email: 'bob@innovate.com',
    phone: '987654321',
    about: 'CEO',
    address: 'HCM',
    has_zalo: 0,
    zalo_reached: 0
  }
];
let mockLeads = defaultLeads.map((lead) => ({ ...lead }));

let invokedCommands: Array<{ command: string; args?: any }> = [];

mock.module('@tauri-apps/api/core', () => ({
  invoke: (command: string, args?: any) => {
    invokedCommands.push({ command, args });
    switch (command) {
      case 'get_smtp_servers': {
        return Promise.reject(new Error('Outreach must not request SMTP passwords'));
      }
      case 'get_managed_smtp_accounts': {
        if (smtpFailuresRemaining > 0) {
          smtpFailuresRemaining -= 1;
          return Promise.reject(new Error('SMTP load failed'));
        }
        return Promise.resolve(
          mockSmtpServers.map((server) => ({
            id: server.id,
            name: server.name,
            host: server.host,
            port: server.port,
            username: server.username,
            fromEmail: server.from_email,
            fromName: server.from_name,
            security: server.security
          }))
        );
      }
      case 'save_smtp_server': {
        const id = args?.server?.id || mockSmtpServers.length + 1;
        const newSmtp = {
          ...args?.server,
          id,
          created_at: '2026-07-24T00:00:00Z'
        };
        if (!args?.server?.id) {
          mockSmtpServers.push(newSmtp);
        } else {
          const idx = mockSmtpServers.findIndex(s => s.id === args.server.id);
          if (idx !== -1) mockSmtpServers[idx] = newSmtp;
        }
        return Promise.resolve(id);
      }
      case 'delete_smtp_server': {
        const idx = mockSmtpServers.findIndex(s => s.id === args?.id);
        if (idx !== -1) mockSmtpServers.splice(idx, 1);
        return Promise.resolve();
      }
      case 'test_smtp_connection': {
        if (args?.host === 'smtp.fail.com') {
          return Promise.reject('Connection timed out');
        }
        return Promise.resolve(`Successfully connected to ${args?.host}:${args?.port}`);
      }
      case 'get_email_templates': {
        return Promise.resolve([...mockEmailTemplates]);
      }
      case 'get_outreach_leads_page': {
        const limit = Number(args?.limit || 50);
        const cursor = args?.cursor == null ? null : Number(args.cursor);
        const matching = [...mockLeads]
          .filter((lead) => cursor === null || lead.id < cursor)
          .sort((left, right) => right.id - left.id);
        const items = matching.slice(0, limit);
        return Promise.resolve({
          items,
          total: mockLeads.length,
          next_cursor: matching.length > limit ? items.at(-1)?.id ?? null : null
        });
      }
      case 'resolve_outreach_lead_ids': {
        const excluded = new Set<number>(args?.excludedLeadIds || []);
        return Promise.resolve(
          mockLeads
            .map((lead) => lead.id)
            .filter((leadId) => !excluded.has(leadId))
            .sort((left, right) => right - left)
        );
      }
      case 'create_email_template': {
        const newTpl = {
          id: mockEmailTemplates.length + 1,
          name: args?.name,
          subject: args?.subject,
          content: args?.content,
          created_at: '2026-07-24T00:00:00Z'
        };
        mockEmailTemplates.push(newTpl);
        return Promise.resolve(newTpl.id);
      }
      case 'update_email_template': {
        const tplIdx = mockEmailTemplates.findIndex(t => t.id === args?.id);
        if (tplIdx !== -1) {
          mockEmailTemplates[tplIdx] = {
            ...mockEmailTemplates[tplIdx],
            name: args?.name,
            subject: args?.subject,
            content: args?.content
          };
        }
        return Promise.resolve();
      }
      case 'delete_email_template': {
        const idx = mockEmailTemplates.findIndex(t => t.id === args?.id);
        if (idx !== -1) mockEmailTemplates.splice(idx, 1);
        return Promise.resolve();
      }
      case 'get_leads': {
        return Promise.resolve([...mockLeads]);
      }
      case 'get_categories': {
        return Promise.resolve([...mockCategories]);
      }
      case 'get_campaign_leads': {
        return Promise.resolve([...mockLeads]);
      }
      case 'send_email_outreach': {
        return Promise.resolve();
      }
      default: {
        return Promise.resolve();
      }
    }
  }
}));

mock.module('@tauri-apps/api/event', () => ({
  listen: (eventName: string, listener: EventListener) => {
    eventListeners.set(eventName, listener);
    return Promise.resolve(() => {
      eventListeners.delete(eventName);
    });
  }
}));

import EmailOutreach from '../EmailOutreach.svelte';
import { notifyLeadsChanged } from '$lib/services/crmService';

beforeEach(() => {
  mockCategories = [{ id: 1, name: 'VIP Clients' }];
  mockLeads = defaultLeads.map((lead) => ({ ...lead }));
  smtpFailuresRemaining = 0;
});

afterEach(() => {
  cleanup();
  eventListeners.clear();
  mockSmtpServers.length = 0;
  mockEmailTemplates.length = 0;
  invokedCommands.length = 0;
});

test('renders EmailOutreach section headings and initial state', async () => {
  render(EmailOutreach);

  expect(await screen.findByText('Available Email Accounts')).toBeTruthy();
  expect(await screen.findByText('Email Templates')).toBeTruthy();
  expect(await screen.findByText('Campaign Execution')).toBeTruthy();
});

test('uses redacted SMTP accounts and delegates management to Settings', async () => {
  mockSmtpServers.push({
    id: 7,
    name: 'Primary SMTP',
    host: 'smtp.example.com',
    port: 587,
    username: 'sender@example.com',
    password: 'must-never-reach-outreach',
    from_email: 'sender@example.com',
    from_name: 'Sales Team',
    security: 'STARTTLS',
    created_at: '2026-08-01'
  });
  const onOpenSettings = mock(() => {});
  render(EmailOutreach, { props: { activeTab: 'email_outreach', onOpenSettings } });

  expect((await screen.findAllByText('Primary SMTP')).length).toBeGreaterThan(0);
  await fireEvent.click(screen.getByRole('button', { name: /Manage accounts in Settings/i }));

  const count = (command: string) => invokedCommands.filter((entry) => entry.command === command).length;
  expect(onOpenSettings).toHaveBeenCalledTimes(1);
  expect(count('get_managed_smtp_accounts')).toBe(1);
  expect(count('get_smtp_servers')).toBe(0);
  expect(count('save_smtp_server')).toBe(0);
  expect(count('delete_smtp_server')).toBe(0);
  expect(screen.queryByText('must-never-reach-outreach')).toBeNull();
});

test('Email template modal controls - opens modal, inserts variable badges into subject and content, and creates template', async () => {
  render(EmailOutreach);

  const addTplBtn = await screen.findByRole('button', { name: /New Template/i });
  await fireEvent.click(addTplBtn);

  expect(await screen.findByText(/Create Email Template/i)).toBeTruthy();

  const nameInput = screen.getByLabelText(/Template Name/i);
  const subjectInput = screen.getByLabelText(/Subject Line/i);
  const contentInput = screen.getByLabelText(/^Content$/i);

  await fireEvent.input(nameInput, { target: { value: 'Welcome Email' } });
  await fireEvent.input(subjectInput, { target: { value: 'Hello ' } });
  await fireEvent.input(contentInput, { target: { value: 'Hi ' } });

  // Click variable badge buttons
  const nameBadge = screen.getByRole('button', { name: /\+ {{name}}/i });
  await fireEvent.click(nameBadge);

  const companyBadge = screen.getByRole('button', { name: /\+ {{company}}/i });
  await fireEvent.click(companyBadge);

  const saveTplBtn = screen.getByRole('button', { name: /Save Template/i });
  await fireEvent.click(saveTplBtn);
  await tick();
  await new Promise((r) => setTimeout(r, 100));

  const matchedTpl = await screen.findAllByText(/Welcome Email/i);
  expect(matchedTpl.length).toBeGreaterThan(0);
});

test('Lead selection and triggers Email Campaign execution with logs and progress', async () => {
  mockSmtpServers.length = 0;
  mockSmtpServers.push({
    id: 1,
    name: 'Default SMTP',
    host: 'smtp.example.com',
    port: 587,
    username: 'user',
    password: 'pass',
    from_email: 'sales@example.com',
    from_name: 'Sales',
    security: 'STARTTLS',
    created_at: '2026-07-24'
  });

  mockEmailTemplates.length = 0;
  mockEmailTemplates.push({
    id: 1,
    name: 'Introduction',
    subject: 'Hello {{name}} from {{company}}',
    content: 'Dear {{name}},\nWe saw your company {{company}}...',
    created_at: '2026-07-24'
  });

  render(EmailOutreach);

  expect(await screen.findByText(/Alice Johnson/i)).toBeTruthy();
  await fireEvent.click(await screen.findByText(/Bob Smith/i));

  const smtpCards = await screen.findAllByText(/Default SMTP/i);
  await fireEvent.click(smtpCards[0]);

  const tplCards = await screen.findAllByText(/Introduction/i);
  await fireEvent.click(tplCards[0]);

  // Trigger Campaign
  const startCampaignBtn = await screen.findByRole('button', { name: /Launch Email Campaign/i });
  await fireEvent.click(startCampaignBtn);
  await new Promise((r) => setTimeout(r, 300));

  await waitFor(() => {
    const sendCmd = invokedCommands.find(c => c.command === 'send_email_outreach');
    expect(sendCmd).toBeTruthy();
  });

  const sendCmd = invokedCommands.find(c => c.command === 'send_email_outreach');
  const resolverCmd = invokedCommands.find(c => c.command === 'resolve_outreach_lead_ids');
  expect(resolverCmd?.args.excludedLeadIds).toEqual([2]);
  expect(resolverCmd?.args.channel).toBe('email');
  expect(sendCmd?.args.smtpId).toBe(1);
  expect(sendCmd?.args.templateId).toBe(1);
  expect(sendCmd?.args.leadIds).toEqual([1]);

  const logListener = eventListeners.get('email-outreach-log');
  const progressListener = eventListeners.get('email-outreach-progress');
  const completeListener = eventListeners.get('email-outreach-complete');

  expect(logListener).toBeTruthy();
  expect(progressListener).toBeTruthy();
  expect(completeListener).toBeTruthy();

  logListener!({ payload: { message: 'Successfully sent email to Alice Johnson (alice@techcorp.com)' } });
  progressListener!({ payload: { progress: 50.0, status: 'running', message: 'Processed 1/2 leads' } });
  completeListener!({ payload: { total_sent: 2, message: 'Email outreach completed. 2 emails sent.' } });
  await tick();
  await new Promise((r) => setTimeout(r, 100));

  const matchedLog = await screen.findAllByText(/Successfully sent email to Alice Johnson/i);
  expect(matchedLog.length).toBeGreaterThan(0);

  const matchedComplete = await screen.findAllByText(/Email outreach completed/i);
  expect(matchedComplete.length).toBeGreaterThan(0);
});

test('does not re-fetch Email outreach data on clean tab switch without background events', async () => {
  const view = render(EmailOutreach, { props: { activeTab: 'dashboard' } });
  await view.rerender({ activeTab: 'email_outreach' });
  await new Promise((resolve) => setTimeout(resolve, 150));

  const count = (command: string) => invokedCommands.filter((entry) => entry.command === command).length;
  const initialCounts = {
    categories: count('get_categories'),
    smtp: count('get_managed_smtp_accounts'),
    templates: count('get_email_templates'),
    leads: count('get_outreach_leads_page')
  };

  await view.rerender({ activeTab: 'dashboard' });
  await view.rerender({ activeTab: 'email_outreach' });
  await new Promise((resolve) => setTimeout(resolve, 100));

  expect(count('get_categories')).toBe(initialCounts.categories);
  expect(count('get_managed_smtp_accounts')).toBe(initialCounts.smtp);
  expect(count('get_email_templates')).toBe(initialCounts.templates);
  expect(count('get_outreach_leads_page')).toBe(initialCounts.leads);
});

test('performs one base refresh when mounted active', async () => {
  render(EmailOutreach, { props: { activeTab: 'email_outreach' } });
  await new Promise((resolve) => setTimeout(resolve, 150));

  const count = (command: string) => invokedCommands.filter((entry) => entry.command === command).length;
  expect(count('get_categories')).toBe(1);
  expect(count('get_managed_smtp_accounts')).toBe(1);
  expect(count('get_email_templates')).toBe(1);
  expect(count('get_outreach_leads_page')).toBe(1);
});

test('retries a failed initial SMTP load after tab revisit', async () => {
  smtpFailuresRemaining = 1;
  const view = render(EmailOutreach, { props: { activeTab: 'dashboard' } });
  const count = (command: string) => invokedCommands.filter((entry) => entry.command === command).length;

  await view.rerender({ activeTab: 'email_outreach' });
  await new Promise((resolve) => setTimeout(resolve, 100));
  const callsAfterFailure = count('get_managed_smtp_accounts');

  await view.rerender({ activeTab: 'dashboard' });
  await view.rerender({ activeTab: 'email_outreach' });
  await new Promise((resolve) => setTimeout(resolve, 150));

  expect(count('get_managed_smtp_accounts')).toBeGreaterThan(callsAfterFailure);
});

test('refreshes CRM data without reloading SMTP servers or templates after lead changes', async () => {
  const view = render(EmailOutreach, { props: { activeTab: 'dashboard' } });
  const count = (command: string) => invokedCommands.filter((entry) => entry.command === command).length;

  await view.rerender({ activeTab: 'email_outreach' });
  await new Promise((resolve) => setTimeout(resolve, 150));
  const initialSmtpCount = count('get_managed_smtp_accounts');
  const initialTemplateCount = count('get_email_templates');
  const initialLeadCount = count('get_outreach_leads_page');

  notifyLeadsChanged();
  await new Promise((resolve) => setTimeout(resolve, 450));

  expect(count('get_managed_smtp_accounts')).toBe(initialSmtpCount);
  expect(count('get_email_templates')).toBe(initialTemplateCount);
  expect(count('get_outreach_leads_page')).toBeGreaterThan(initialLeadCount);
});

test('refreshes stale Email lead data after leaving during the realtime debounce', async () => {
  const view = render(EmailOutreach, { props: { activeTab: 'email_outreach' } });
  const count = (command: string) => invokedCommands.filter((entry) => entry.command === command).length;
  await new Promise((resolve) => setTimeout(resolve, 150));
  const initialLeadCount = count('get_outreach_leads_page');

  notifyLeadsChanged();
  await view.rerender({ activeTab: 'dashboard' });
  await new Promise((resolve) => setTimeout(resolve, 350));
  expect(count('get_outreach_leads_page')).toBe(initialLeadCount);

  await view.rerender({ activeTab: 'email_outreach' });
  await new Promise((resolve) => setTimeout(resolve, 100));
  expect(count('get_outreach_leads_page')).toBeGreaterThan(initialLeadCount);
});

test('loads all leads when no category is selected', async () => {
  mockCategories = [];
  const view = render(EmailOutreach, { props: { activeTab: 'dashboard' } });
  const count = (command: string) => invokedCommands.filter((entry) => entry.command === command).length;

  await view.rerender({ activeTab: 'email_outreach' });
  expect(await screen.findByText(/Alice Johnson/)).toBeTruthy();

  expect(count('get_outreach_leads_page')).toBe(1);
  expect(count('get_campaign_leads')).toBe(0);
});

test('renders only one 50-lead page and navigates by cursor', async () => {
  mockLeads = Array.from({ length: 125 }, (_, index) => ({
    ...defaultLeads[0],
    id: index + 1,
    name: `Bulk Email Lead ${index + 1}`,
    email: `email${index + 1}@example.com`
  }));

  render(EmailOutreach, { props: { activeTab: 'email_outreach' } });

  expect(await screen.findByText(/Bulk Email Lead 125/)).toBeTruthy();
  expect(screen.queryAllByText(/Bulk Email Lead/)).toHaveLength(50);

  await fireEvent.click(screen.getByRole('button', { name: 'Next leads page' }));

  expect(await screen.findByText(/Bulk Email Lead 75/)).toBeTruthy();
  expect(screen.queryAllByText(/Bulk Email Lead/)).toHaveLength(50);

  const callsAfterNext = invokedCommands.filter((entry) => entry.command === 'get_outreach_leads_page').length;
  await fireEvent.click(screen.getByRole('button', { name: 'Previous leads page' }));

  expect(await screen.findByText(/Bulk Email Lead 125/)).toBeTruthy();
  expect(invokedCommands.filter((entry) => entry.command === 'get_outreach_leads_page').length).toBe(callsAfterNext);
}, 20000);
