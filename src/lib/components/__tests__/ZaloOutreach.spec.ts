/// <reference types="bun-types" />
import { afterEach, beforeEach, expect, mock, test } from 'bun:test';

type EventListener = (event: { payload: unknown }) => void;

const eventListeners = new Map<string, EventListener>();
let mockSessions: Array<{
  imei: string;
  user_agent: string;
  cookies_json: string;
  language: string;
}> = [];
let mockCategories: Array<{ id: number; name: string }> = [];
let mockTemplates: Array<{ id: number; name: string; content: string }> = [];
const defaultLead = {
  id: 1,
  name: 'Probe Lead',
  company: 'Probe Company',
  status: 'New',
  value: 1000,
  last_activity: '2026-07-30',
  owner: 'Sales',
  email: 'probe@example.com',
  phone: '0900000000',
  about: null
};
let mockLeads = [{ ...defaultLead }];
let sessionFailuresRemaining = 0;
const commandCounts: Record<string, number> = {};
let invokedCommands: Array<{ command: string; args?: Record<string, any> }> = [];

function countCommand(command: string): number {
  return commandCounts[command] || 0;
}

mock.module('@tauri-apps/api/core', () => ({
  invoke: (command: string, args?: Record<string, any>) => {
    commandCounts[command] = countCommand(command) + 1;
    invokedCommands.push({ command, args });
    switch (command) {
      case 'get_zalo_sessions':
        if (sessionFailuresRemaining > 0) {
          sessionFailuresRemaining -= 1;
          return Promise.reject(new Error('session load failed'));
        }
        return Promise.resolve(mockSessions);
      case 'verify_all_zalo_sessions':
        return Promise.resolve(
          mockSessions.map((s) => ({
            imei: s.imei,
            is_valid: true,
            display_name: `User ${s.imei}`,
            avatar: undefined,
            error_message: undefined
          }))
        );
      case 'verify_zalo_session':
        return Promise.resolve({
          imei: args?.imei || '',
          is_valid: true,
          display_name: `Verified ${args?.imei}`,
          avatar: undefined,
          error_message: undefined
        });
      case 'get_outreach_templates':
        return Promise.resolve([...mockTemplates]);
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
      case 'get_leads':
      case 'get_campaign_leads':
        return Promise.resolve([...mockLeads]);
      case 'get_categories':
        return Promise.resolve([...mockCategories]);
      case 'start_zalo_qr_login':
        return Promise.resolve();
      default:
        return Promise.resolve();
    }
  }
}));

mock.module('@tauri-apps/api/event', () => ({
  listen: (eventName: string, listener: EventListener) => {
    eventListeners.set(eventName, listener);
    return Promise.resolve(() => eventListeners.delete(eventName));
  }
}));

import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import ZaloOutreach from '../ZaloOutreach.svelte';
import { notifyLeadsChanged } from '$lib/services/crmService';

async function waitForListener(eventName: string): Promise<EventListener> {
  for (let attempt = 0; attempt < 10; attempt += 1) {
    const listener = eventListeners.get(eventName);
    if (listener) {
      return listener;
    }
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
  }

  throw new Error(`Listener ${eventName} was not registered`);
}

beforeEach(() => {
  mockSessions = [];
  mockCategories = [];
  mockTemplates = [];
  mockLeads = [{ ...defaultLead }];
  sessionFailuresRemaining = 0;
  invokedCommands = [];
  for (const command of Object.keys(commandCounts)) delete commandCounts[command];
});

afterEach(() => {
  cleanup();
  eventListeners.clear();
});

test('shows a terminal QR login error instead of leaving the modal pending', async () => {
  render(ZaloOutreach);

  await fireEvent.click(screen.getByRole('button', { name: /Add Session/ }));
  const errorListener = await waitForListener('zalo-qr-error');

  errorListener({
    payload: {
      message: 'Zalo confirmed the scan but did not finish creating a session. Please try again.'
    }
  });

  expect(
    await screen.findByText('Zalo confirmed the scan but did not finish creating a session. Please try again.')
  ).toBeTruthy();
});

test('loads the saved session and closes the QR modal after login succeeds', async () => {
  render(ZaloOutreach);

  await fireEvent.click(screen.getByRole('button', { name: /Add Session/ }));
  const successListener = await waitForListener('zalo-qr-success');
  mockSessions = [{
    imei: 'device-imei-123',
    user_agent: 'Mozilla/5.0',
    cookies_json: '[]',
    language: 'vi'
  }];

  successListener({ payload: { imei: 'device-imei-123' } });

  expect((await screen.findAllByText(/device-imei-123/, {}, { timeout: 2500 })).length).toBeGreaterThan(0);
  expect(screen.queryByText('Zalo QR Login')).toBeNull();
});

test('verifies Zalo sessions only after the user clicks Check All', async () => {
  mockSessions = [{
    imei: 'device-imei-456',
    user_agent: 'Mozilla/5.0',
    cookies_json: '[]',
    language: 'vi'
  }];

  render(ZaloOutreach, { props: { activeTab: 'outreach' } });
  await new Promise((resolve) => setTimeout(resolve, 150));

  expect(countCommand('verify_all_zalo_sessions')).toBe(0);

  await fireEvent.click(screen.getByRole('button', { name: /Check All/i }));

  expect((await screen.findAllByText(/User device-imei-456/)).length).toBeGreaterThan(0);
});

test('does not re-fetch Zalo outreach data on clean tab switch without background events', async () => {
  mockSessions = [{
    imei: 'device-imei-789',
    user_agent: 'Mozilla/5.0',
    cookies_json: '[]',
    language: 'vi'
  }];

  const view = render(ZaloOutreach, { props: { activeTab: 'dashboard' } });
  await view.rerender({ activeTab: 'outreach' });
  await new Promise((resolve) => setTimeout(resolve, 200));

  const initialCounts = {
    categories: countCommand('get_categories'),
    sessions: countCommand('get_zalo_sessions'),
    templates: countCommand('get_outreach_templates'),
    leads: countCommand('get_outreach_leads_page')
  };

  await view.rerender({ activeTab: 'dashboard' });
  await view.rerender({ activeTab: 'outreach' });
  await new Promise((resolve) => setTimeout(resolve, 100));

  expect(countCommand('get_categories')).toBe(initialCounts.categories);
  expect(countCommand('get_zalo_sessions')).toBe(initialCounts.sessions);
  expect(countCommand('get_outreach_templates')).toBe(initialCounts.templates);
  expect(countCommand('get_outreach_leads_page')).toBe(initialCounts.leads);
});

test('performs one base refresh when mounted active', async () => {
  render(ZaloOutreach, { props: { activeTab: 'outreach' } });
  await new Promise((resolve) => setTimeout(resolve, 150));

  expect(countCommand('get_categories')).toBe(1);
  expect(countCommand('get_zalo_sessions')).toBe(1);
  expect(countCommand('get_outreach_templates')).toBe(1);
  expect(countCommand('get_outreach_leads_page')).toBe(1);
});

test('retries a failed initial session load after tab revisit', async () => {
  sessionFailuresRemaining = 1;
  const view = render(ZaloOutreach, { props: { activeTab: 'dashboard' } });

  await view.rerender({ activeTab: 'outreach' });
  await new Promise((resolve) => setTimeout(resolve, 100));
  const callsAfterFailure = countCommand('get_zalo_sessions');

  await view.rerender({ activeTab: 'dashboard' });
  await view.rerender({ activeTab: 'outreach' });
  await new Promise((resolve) => setTimeout(resolve, 150));

  expect(countCommand('get_zalo_sessions')).toBeGreaterThan(callsAfterFailure);
});

test('refreshes CRM data without re-verifying sessions after lead changes', async () => {
  mockSessions = [{
    imei: 'device-imei-event',
    user_agent: 'Mozilla/5.0',
    cookies_json: '[]',
    language: 'vi'
  }];
  const view = render(ZaloOutreach, { props: { activeTab: 'dashboard' } });

  await view.rerender({ activeTab: 'outreach' });
  await new Promise((resolve) => setTimeout(resolve, 200));
  const initialVerificationCount = countCommand('verify_all_zalo_sessions');
  const initialLeadCount = countCommand('get_outreach_leads_page');

  notifyLeadsChanged();
  await new Promise((resolve) => setTimeout(resolve, 450));

  expect(countCommand('verify_all_zalo_sessions')).toBe(initialVerificationCount);
  expect(countCommand('get_outreach_leads_page')).toBeGreaterThan(initialLeadCount);
});

test('refreshes stale Zalo lead data after leaving during the realtime debounce', async () => {
  const view = render(ZaloOutreach, { props: { activeTab: 'outreach' } });
  await new Promise((resolve) => setTimeout(resolve, 150));
  const initialLeadCount = countCommand('get_outreach_leads_page');

  notifyLeadsChanged();
  await view.rerender({ activeTab: 'dashboard' });
  await new Promise((resolve) => setTimeout(resolve, 350));
  expect(countCommand('get_outreach_leads_page')).toBe(initialLeadCount);

  await view.rerender({ activeTab: 'outreach' });
  await new Promise((resolve) => setTimeout(resolve, 100));
  expect(countCommand('get_outreach_leads_page')).toBeGreaterThan(initialLeadCount);
});

test('loads all leads when no category is selected', async () => {
  const view = render(ZaloOutreach, { props: { activeTab: 'dashboard' } });

  await view.rerender({ activeTab: 'outreach' });
  expect(await screen.findByText('Probe Lead')).toBeTruthy();

  expect(countCommand('get_outreach_leads_page')).toBe(1);
  expect(countCommand('get_campaign_leads')).toBe(0);
});

test('renders only one 50-lead page and navigates by cursor', async () => {
  mockLeads = Array.from({ length: 125 }, (_, index) => ({
    ...defaultLead,
    id: index + 1,
    name: `Bulk Zalo Lead ${index + 1}`,
    email: `zalo${index + 1}@example.com`
  }));

  render(ZaloOutreach, { props: { activeTab: 'outreach' } });

  expect(await screen.findByText('Bulk Zalo Lead 125')).toBeTruthy();
  expect(screen.queryAllByText(/Bulk Zalo Lead/)).toHaveLength(50);

  await fireEvent.click(screen.getByRole('button', { name: 'Next leads page' }));

  expect(await screen.findByText('Bulk Zalo Lead 75')).toBeTruthy();
  expect(screen.queryAllByText(/Bulk Zalo Lead/)).toHaveLength(50);

  const callsAfterNext = countCommand('get_outreach_leads_page');
  await fireEvent.click(screen.getByRole('button', { name: 'Previous leads page' }));

  expect(await screen.findByText('Bulk Zalo Lead 125')).toBeTruthy();
  expect(countCommand('get_outreach_leads_page')).toBe(callsAfterNext);
});

test('resolves all matching Zalo leads with row exclusions before launch', async () => {
  mockSessions = [{
    imei: 'campaign-imei',
    user_agent: 'Mozilla/5.0',
    cookies_json: '[]',
    language: 'vi'
  }];
  mockTemplates = [{ id: 7, name: 'Campaign Template', content: 'Hello' }];
  mockLeads = [
    { ...defaultLead, id: 1, name: 'Included Lead' },
    { ...defaultLead, id: 2, name: 'Excluded Lead' }
  ];

  render(ZaloOutreach, { props: { activeTab: 'outreach' } });

  await fireEvent.click(await screen.findByText('Excluded Lead'));
  await fireEvent.click(await screen.findByRole('button', { name: /Launch Zalo Campaign/i }));
  await new Promise((resolve) => setTimeout(resolve, 50));

  const resolverCommand = invokedCommands.find((entry) => entry.command === 'resolve_outreach_lead_ids');
  const campaignCommand = invokedCommands.find((entry) => entry.command === 'start_zalo_outreach');

  expect(resolverCommand?.args?.excludedLeadIds).toEqual([2]);
  expect(resolverCommand?.args?.channel).toBe('zalo');
  expect(campaignCommand?.args?.leadIds).toEqual([1]);
});
