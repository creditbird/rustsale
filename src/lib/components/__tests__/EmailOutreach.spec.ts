/// <reference types="bun-types" />
import { afterEach, expect, mock, test } from 'bun:test';
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

let mockLeads = [
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

let invokedCommands: Array<{ command: string; args?: any }> = [];

mock.module('@tauri-apps/api/core', () => ({
  invoke: (command: string, args?: any) => {
    invokedCommands.push({ command, args });
    switch (command) {
      case 'get_smtp_servers': {
        return Promise.resolve([...mockSmtpServers]);
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
        return Promise.resolve([{ id: 1, name: 'VIP Clients' }]);
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

afterEach(() => {
  cleanup();
  eventListeners.clear();
  mockSmtpServers.length = 0;
  mockEmailTemplates.length = 0;
  invokedCommands.length = 0;
});

test('renders EmailOutreach section headings and initial state', async () => {
  render(EmailOutreach);

  expect(await screen.findByRole('heading', { name: 'SMTP Configuration' })).toBeTruthy();
  expect(await screen.findByRole('heading', { name: 'Email Templates' })).toBeTruthy();
  expect(await screen.findByRole('heading', { name: 'Campaign Execution' })).toBeTruthy();
});

test('SMTP server modal controls - opens modal, saves new SMTP server and tests connection', async () => {
  render(EmailOutreach);

  // Open Add SMTP Modal
  const addSmtpBtn = await screen.findByRole('button', { name: /Add SMTP Server/i });
  await fireEvent.click(addSmtpBtn);

  expect(await screen.findByRole('heading', { level: 3, name: /Add SMTP Server/i })).toBeTruthy();

  // Fill in form fields
  const nameInput = screen.getByLabelText(/Server Name/i);
  const hostInput = screen.getByLabelText(/SMTP Host/i);
  const portInput = screen.getByLabelText(/SMTP Port/i);
  const usernameInput = screen.getByLabelText(/Username/i);
  const passwordInput = screen.getByLabelText(/Password/i);
  const fromEmailInput = screen.getByLabelText(/From Email/i);
  const fromNameInput = screen.getByLabelText(/From Name/i);

  await fireEvent.input(nameInput, { target: { value: 'Primary SMTP' } });
  await fireEvent.input(hostInput, { target: { value: 'smtp.gmail.com' } });
  await fireEvent.input(portInput, { target: { value: '587' } });
  await fireEvent.input(usernameInput, { target: { value: 'user@gmail.com' } });
  await fireEvent.input(passwordInput, { target: { value: 'secretpass' } });
  await fireEvent.input(fromEmailInput, { target: { value: 'outreach@company.com' } });
  await fireEvent.input(fromNameInput, { target: { value: 'Sales Team' } });

  // Test Connection
  const testBtn = screen.getByRole('button', { name: /Test Connection/i });
  await fireEvent.click(testBtn);

  expect(await screen.findByText(/Successfully connected to smtp.gmail.com:587/i)).toBeTruthy();
  await tick();
  await new Promise((r) => setTimeout(r, 50));

  // Save SMTP Server
  const saveBtn = screen.getByRole('button', { name: /Save Server/i });
  await fireEvent.click(saveBtn);
  await tick();
  await new Promise((r) => setTimeout(r, 100));

  const matchedSmtp = await screen.findAllByText(/Primary SMTP/i);
  expect(matchedSmtp.length).toBeGreaterThan(0);
});

test('Email template modal controls - opens modal, inserts variable badges into subject and content, and creates template', async () => {
  render(EmailOutreach);

  const addTplBtn = await screen.findByRole('button', { name: /Create Template/i });
  await fireEvent.click(addTplBtn);

  expect(await screen.findByText(/Create Email Template/i)).toBeTruthy();

  const nameInput = screen.getByLabelText(/Template Name/i);
  const subjectInput = screen.getByLabelText(/Subject Line/i);
  const contentInput = screen.getByLabelText(/Email Content/i);

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
  expect(await screen.findByText(/Bob Smith/i)).toBeTruthy();

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
  expect(sendCmd?.args.smtp_id).toBe(1);
  expect(sendCmd?.args.template_id).toBe(1);
  expect(sendCmd?.args.lead_ids.length).toBe(2);

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
