/// <reference types="bun-types" />
import { afterEach, expect, mock, test } from 'bun:test';

type EventListener = (event: { payload: unknown }) => void;

const eventListeners = new Map<string, EventListener>();
let mockSessions: Array<{
  imei: string;
  user_agent: string;
  cookies_json: string;
  language: string;
}> = [];

mock.module('@tauri-apps/api/core', () => ({
  invoke: (command: string, args?: { imei?: string }) => {
    switch (command) {
      case 'get_zalo_sessions':
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
      case 'get_leads':
      case 'get_categories':
        return Promise.resolve([]);
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

afterEach(() => {
  cleanup();
  eventListeners.clear();
  mockSessions = [];
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

  expect((await screen.findAllByText(/device-imei-123/)).length).toBeGreaterThan(0);
  expect(screen.queryByText('Zalo QR Login')).toBeNull();
});

test('verifies all Zalo sessions and displays health badges and display names', async () => {
  mockSessions = [{
    imei: 'device-imei-456',
    user_agent: 'Mozilla/5.0',
    cookies_json: '[]',
    language: 'vi'
  }];

  render(ZaloOutreach);

  expect(await screen.findByText(/User device-imei-456/)).toBeTruthy();
});
