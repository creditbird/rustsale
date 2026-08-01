/// <reference types="bun-types" />
import { afterEach, beforeEach, describe, expect, mock, test } from 'bun:test';

type EventListener = (event: { payload: any }) => void;

const eventListeners = new Map<string, EventListener>();
let invokedCommands: Array<{ command: string; args?: Record<string, any> }> = [];

const zaloAccounts = [
  {
    accountKey: 'imei-1',
    displayName: 'Zalo Sales',
    avatar: null,
    authStatus: 'valid',
    authCheckedAt: '2026-08-01T08:00:00Z',
    authError: null,
    runtimeStatus: 'disconnected',
    lastConnectedAt: '2026-08-01T07:00:00Z',
    lastError: 'Realtime listener stopped'
  }
];

const smtpAccounts = [
  {
    id: 7,
    name: 'Primary SMTP',
    host: 'smtp.example.com',
    port: 587,
    username: 'sender@example.com',
    fromEmail: 'sender@example.com',
    fromName: 'Sales Team',
    security: 'STARTTLS'
  }
];

mock.module('@tauri-apps/api/core', () => ({
  invoke: async (command: string, args?: Record<string, any>) => {
    invokedCommands.push({ command, args });
    switch (command) {
      case 'get_managed_zalo_accounts':
        return structuredClone(zaloAccounts);
      case 'get_managed_smtp_accounts':
        return structuredClone(smtpAccounts);
      case 'verify_zalo_session':
        return {
          imei: args?.imei,
          is_valid: true,
          display_name: 'Zalo Sales'
        };
      case 'save_smtp_server':
        return 7;
      case 'test_smtp_connection':
        return 'Connected';
      default:
        return undefined;
    }
  }
}));

mock.module('@tauri-apps/api/event', () => ({
  listen: async (eventName: string, listener: EventListener) => {
    eventListeners.set(eventName, listener);
    return () => eventListeners.delete(eventName);
  }
}));

import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import Settings from '../Settings.svelte';

async function selectTab(name: RegExp) {
  await fireEvent.click(screen.getByRole('tab', { name }));
}

async function waitForListener(eventName: string) {
  await waitFor(() => expect(eventListeners.has(eventName)).toBe(true));
  return eventListeners.get(eventName)!;
}

beforeEach(() => {
  invokedCommands = [];
  eventListeners.clear();
});

afterEach(() => {
  cleanup();
  eventListeners.clear();
});

describe('Settings', () => {
  test('renders Zalo and Email account tabs with disabled future channels', async () => {
    render(Settings);

    expect(screen.getByRole('tab', { name: /Zalo/i })).toBeTruthy();
    expect(screen.getByRole('tab', { name: /Email/i })).toBeTruthy();

    const facebookTab = screen.getByRole('tab', { name: /Facebook.*Coming soon/i });
    const whatsappTab = screen.getByRole('tab', { name: /WhatsApp.*Coming soon/i });
    expect((facebookTab as HTMLButtonElement).disabled).toBe(true);
    expect((whatsappTab as HTMLButtonElement).disabled).toBe(true);
  });

  test('shows authentication and realtime runtime as independent statuses', async () => {
    render(Settings);

    expect(await screen.findByText('Authentication Valid')).toBeTruthy();
    expect(screen.getByText('Runtime Disconnected')).toBeTruthy();
    expect(screen.getByText('Realtime listener stopped')).toBeTruthy();
  });

  test('reconnects a disconnected Zalo account through the supervisor command', async () => {
    render(Settings);

    await fireEvent.click(await screen.findByRole('button', { name: /Reconnect Zalo Sales/i }));

    await waitFor(() => {
      expect(invokedCommands).toContainEqual({
        command: 'restart_conversation_account',
        args: { providerAccountKey: 'imei-1' }
      });
    });
  });

  test('editing SMTP sends an empty password unless a replacement is entered', async () => {
    render(Settings);
    await selectTab(/Email/i);

    await fireEvent.click(await screen.findByRole('button', { name: /Edit Primary SMTP/i }));
    const password = screen.getByLabelText(/Replacement password/i) as HTMLInputElement;
    expect(password.value).toBe('');

    await fireEvent.click(screen.getByRole('button', { name: /Save account/i }));

    await waitFor(() => {
      const save = invokedCommands.find((entry) => entry.command === 'save_smtp_server');
      expect(save?.args?.server.password).toBe('');
      expect(save?.args?.server.id).toBe(7);
    });
  });

  test('shows QR failures as a terminal error state', async () => {
    render(Settings);

    await fireEvent.click(await screen.findByRole('button', { name: /Add Zalo account/i }));
    const errorListener = await waitForListener('zalo-qr-error');
    errorListener({ payload: { message: 'QR login expired. Start again.' } });

    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain('QR login expired. Start again.');
    expect(screen.getByRole('button', { name: /Try again/i })).toBeTruthy();
  });
});
