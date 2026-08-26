import { afterEach, beforeEach, describe, expect, mock, test } from 'bun:test';
import { setLocale } from '$lib/i18n';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import WhatsAppAccountsSettings from '../WhatsAppAccountsSettings.svelte';
import type { ConnectedWhatsAppAccount } from '$lib/services/whatsappService';

type EventListener = (event: { payload: any }) => void;

const eventListeners = new Map<string, EventListener>();
let invokedCommands: Array<{ command: string; args?: Record<string, any> }> = [];
let mockAccounts: ConnectedWhatsAppAccount[] = [];
let mockListError: Error | null = null;
let mockDisconnectError: Error | null = null;
let mockStartQrError: Error | null = null;

mock.module('@tauri-apps/api/core', () => ({
  invoke: async (command: string, args?: Record<string, any>) => {
    invokedCommands.push({ command, args });
    switch (command) {
      case 'get_managed_whatsapp_accounts':
        if (mockListError) throw mockListError;
        return structuredClone(mockAccounts);
      case 'start_whatsapp_qr_login':
        if (mockStartQrError) throw mockStartQrError;
        return undefined;
      case 'cancel_whatsapp_qr_login':
        return undefined;
      case 'disconnect_whatsapp_account':
        if (mockDisconnectError) throw mockDisconnectError;
        mockAccounts = mockAccounts.filter((a) => a.id !== args?.channelAccountId);
        return undefined;
      default:
        return undefined;
    }
  }
}));

mock.module('@tauri-apps/api/event', () => ({
  listen: async (eventName: string, listener: EventListener) => {
    eventListeners.set(eventName, listener);
    return () => {
      eventListeners.delete(eventName);
    };
  }
}));

async function waitForListener(eventName: string): Promise<EventListener> {
  await waitFor(() => expect(eventListeners.has(eventName)).toBe(true));
  return eventListeners.get(eventName)!;
}

beforeEach(() => {
  setLocale('en');
  invokedCommands = [];
  eventListeners.clear();
  mockListError = null;
  mockDisconnectError = null;
  mockStartQrError = null;
  mockAccounts = [];
});

afterEach(() => {
  cleanup();
  eventListeners.clear();
});

describe('WhatsAppAccountsSettings', () => {
  test('renders empty state when no accounts exist', async () => {
    mockAccounts = [];
    render(WhatsAppAccountsSettings);

    expect(await screen.findByText('WhatsApp accounts')).toBeTruthy();
    expect(await screen.findByText('No WhatsApp account connected')).toBeTruthy();
    expect(screen.getAllByRole('button', { name: /Add WhatsApp account/i }).length).toBeGreaterThanOrEqual(1);
  });

  test('renders account cards when accounts exist', async () => {
    mockAccounts = [
      {
        id: 1,
        provider_account_key: '84901234567@s.whatsapp.net',
        display_name: 'Sales Support',
        phone_number: '+84901234567'
      },
      {
        id: 2,
        provider_account_key: '84988888888@s.whatsapp.net',
        display_name: null,
        phone_number: '+84988888888'
      }
    ];

    render(WhatsAppAccountsSettings);

    expect(await screen.findByText('Sales Support')).toBeTruthy();
    expect(screen.getAllByText('+84901234567').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('+84988888888').length).toBeGreaterThanOrEqual(1);

    const disconnectButtons = screen.getAllByRole('button', { name: /Disconnect/i });
    expect(disconnectButtons.length).toBe(2);
  });

  test('launches QR pairing modal and calls start_whatsapp_qr_login with attemptId', async () => {
    mockAccounts = [];
    render(WhatsAppAccountsSettings);

    const addBtn = await screen.findByRole('button', { name: /Add WhatsApp account/i });
    await fireEvent.click(addBtn);

    expect(await screen.findByText('Connect WhatsApp account')).toBeTruthy();

    const startCmd = invokedCommands.find((c) => c.command === 'start_whatsapp_qr_login');
    expect(startCmd).toBeTruthy();
    expect(typeof startCmd?.args?.attemptId).toBe('string');
    expect(startCmd?.args?.attemptId.length).toBeGreaterThan(0);
  });

  test('handles whatsapp-qr-code event and displays QR code image and sequence badge', async () => {
    mockAccounts = [];
    render(WhatsAppAccountsSettings);

    const addBtn = await screen.findByRole('button', { name: /Add WhatsApp account/i });
    await fireEvent.click(addBtn);

    const qrListener = await waitForListener('whatsapp-qr-code');
    const startCmd = invokedCommands.find((c) => c.command === 'start_whatsapp_qr_login');
    const attemptId = startCmd!.args!.attemptId;

    qrListener({
      payload: {
        attemptId,
        qrRaw: '2@mock-whatsapp-qr-payload',
        expiresInSec: 30,
        sequence: 1
      }
    });

    expect(await screen.findByAltText('WhatsApp QR Code')).toBeTruthy();
    expect(await screen.findByText(/Code #1/i)).toBeTruthy();
    expect(await screen.findByText(/30s/i)).toBeTruthy();
    expect(
      screen.getByText(/Scan this QR code with WhatsApp on your phone/i)
    ).toBeTruthy();
  });

  test('handles whatsapp-login-success event and reloads accounts', async () => {
    mockAccounts = [];
    render(WhatsAppAccountsSettings);

    const addBtn = await screen.findByRole('button', { name: /Add WhatsApp account/i });
    await fireEvent.click(addBtn);

    const successListener = await waitForListener('whatsapp-login-success');
    const startCmd = invokedCommands.find((c) => c.command === 'start_whatsapp_qr_login');
    const attemptId = startCmd!.args!.attemptId;

    // Simulate account added in backend
    mockAccounts = [
      {
        id: 10,
        provider_account_key: '84999999999@s.whatsapp.net',
        display_name: 'Alice Sales',
        phone_number: '+84999999999'
      }
    ];

    successListener({
      payload: {
        attemptId,
        accountId: 10,
        accountKey: '84999999999@s.whatsapp.net',
        phoneNumber: '+84999999999',
        pushName: 'Alice Sales',
        lid: '12345@lid',
        avatarUrl: null
      }
    });

    expect(await screen.findByText(/Connected as Alice Sales/i)).toBeTruthy();
    expect(screen.getByText(/WhatsApp account paired successfully!/i)).toBeTruthy();

    const listCmdCount = invokedCommands.filter((c) => c.command === 'get_managed_whatsapp_accounts').length;
    expect(listCmdCount).toBeGreaterThanOrEqual(2);
  });

  test('handles whatsapp-login-error event and allows retrying', async () => {
    mockAccounts = [];
    render(WhatsAppAccountsSettings);

    const addBtn = await screen.findByRole('button', { name: /Add WhatsApp account/i });
    await fireEvent.click(addBtn);

    const errorListener = await waitForListener('whatsapp-login-error');
    const startCmd = invokedCommands.find((c) => c.command === 'start_whatsapp_qr_login');
    const attemptId = startCmd!.args!.attemptId;

    errorListener({
      payload: {
        attemptId,
        error: 'Pairing timed out. Please try again.'
      }
    });

    expect(await screen.findByText('Pairing timed out. Please try again.')).toBeTruthy();
    const tryAgainBtn = screen.getByRole('button', { name: /Try again/i });
    expect(tryAgainBtn).toBeTruthy();

    await fireEvent.click(tryAgainBtn);
    const startCmds = invokedCommands.filter((c) => c.command === 'start_whatsapp_qr_login');
    expect(startCmds.length).toBe(2);
  });

  test('closing modal calls cancel_whatsapp_qr_login with attemptId', async () => {
    mockAccounts = [];
    render(WhatsAppAccountsSettings);

    const addBtn = await screen.findByRole('button', { name: /Add WhatsApp account/i });
    await fireEvent.click(addBtn);

    const startCmd = invokedCommands.find((c) => c.command === 'start_whatsapp_qr_login');
    const attemptId = startCmd!.args!.attemptId;

    const cancelBtn = await screen.findByRole('button', { name: /Cancel/i });
    await fireEvent.click(cancelBtn);

    const cancelCmd = invokedCommands.find((c) => c.command === 'cancel_whatsapp_qr_login');
    expect(cancelCmd).toBeTruthy();
    expect(cancelCmd?.args?.attemptId).toBe(attemptId);
  });

  test('disconnects an account after user confirms in dialog', async () => {
    mockAccounts = [
      {
        id: 42,
        provider_account_key: '84911111111@s.whatsapp.net',
        display_name: 'Branch Hotline',
        phone_number: '+84911111111'
      }
    ];

    render(WhatsAppAccountsSettings);

    expect(await screen.findByText('Branch Hotline')).toBeTruthy();

    const disconnectBtn = screen.getByRole('button', { name: /Disconnect Branch Hotline/i });
    await fireEvent.click(disconnectBtn);

    expect(await screen.findByText(/Are you sure you want to disconnect Branch Hotline\?/i)).toBeTruthy();

    // Confirm disconnect in dialog
    const confirmButtons = screen.getAllByRole('button', { name: /^Disconnect$/i });
    const modalConfirmBtn = confirmButtons[confirmButtons.length - 1];
    await fireEvent.click(modalConfirmBtn);

    await waitFor(() => {
      const disconnectCmd = invokedCommands.find((c) => c.command === 'disconnect_whatsapp_account');
      expect(disconnectCmd).toBeTruthy();
      expect(disconnectCmd?.args?.channelAccountId).toBe(42);
    });

    expect(await screen.findByText('No WhatsApp account connected')).toBeTruthy();
  });

  test('refresh button calls get_managed_whatsapp_accounts', async () => {
    mockAccounts = [];
    render(WhatsAppAccountsSettings);

    const refreshBtn = await screen.findByRole('button', { name: /Refresh/i });
    await fireEvent.click(refreshBtn);

    const listCmds = invokedCommands.filter((c) => c.command === 'get_managed_whatsapp_accounts');
    expect(listCmds.length).toBeGreaterThanOrEqual(2);
  });

  test('renders error alert when loading accounts fails', async () => {
    mockListError = new Error('Database connection failed');
    render(WhatsAppAccountsSettings);

    expect(await screen.findByRole('alert')).toBeTruthy();
    expect(screen.getByText('Database connection failed')).toBeTruthy();
  });
});
