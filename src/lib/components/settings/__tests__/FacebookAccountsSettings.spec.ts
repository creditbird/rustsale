import { afterEach, beforeEach, describe, expect, mock, test } from 'bun:test';
import { setLocale } from '$lib/i18n';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import FacebookAccountsSettings from '../FacebookAccountsSettings.svelte';
import type { ManagedFacebookAccount } from '$lib/services/accountSettingsService';

let invokedCommands: Array<{ command: string; args?: Record<string, any> }> = [];
let mockAccounts: ManagedFacebookAccount[] = [];
let mockListError: Error | null = null;
let mockDeleteError: Error | null = null;

mock.module('@tauri-apps/api/core', () => ({
  invoke: async (command: string, args?: Record<string, any>) => {
    invokedCommands.push({ command, args });
    switch (command) {
      case 'get_managed_facebook_accounts':
        if (mockListError) throw mockListError;
        return structuredClone(mockAccounts);
      case 'delete_facebook_account':
        if (mockDeleteError) throw mockDeleteError;
        mockAccounts = mockAccounts.filter((a) => a.pageId !== args?.pageId);
        return undefined;
      default:
        return undefined;
    }
  }
}));

beforeEach(() => {
  setLocale('en');
  invokedCommands = [];
  mockListError = null;
  mockDeleteError = null;
  mockAccounts = [
    {
      channelAccountId: 101,
      pageId: 'page_123',
      pageName: 'RustSale Store',
      avatarUrl: 'https://example.com/logo.png',
      tokenStatus: 'valid',
      grantedScopes: ['pages_messaging', 'pages_read_engagement'],
      runtimeStatus: 'connected',
      lastConnectedAt: '2026-08-23T10:00:00Z',
      lastError: null,
      updatedAt: '2026-08-23T10:00:00Z'
    }
  ];
});

afterEach(() => {
  cleanup();
});

describe('FacebookAccountsSettings', () => {
  test('renders connected fanpages with badges and scopes', async () => {
    render(FacebookAccountsSettings);

    expect(await screen.findByText('RustSale Store')).toBeTruthy();
    expect(screen.getByText('ID: page_123')).toBeTruthy();
    expect(screen.getByText('Token Valid')).toBeTruthy();
    expect(screen.getByText('Runtime Connected')).toBeTruthy();
    expect(screen.getByText('pages_messaging')).toBeTruthy();
    expect(screen.getByText('pages_read_engagement')).toBeTruthy();
  });

  test('renders empty state when no accounts exist', async () => {
    mockAccounts = [];
    render(FacebookAccountsSettings);

    expect(await screen.findByText('No Facebook Pages Connected')).toBeTruthy();
    expect(screen.getByText(/Connect your Facebook Fanpage/i)).toBeTruthy();
  });

  test('opens connect modal and triggers onNavigateToHub', async () => {
    let navigated = false;
    render(FacebookAccountsSettings, {
      props: {
        onNavigateToHub: () => {
          navigated = true;
        }
      }
    });

    await fireEvent.click(await screen.findByRole('button', { name: /Connect Fanpage/i }));

    expect(await screen.findByText('How connection works:')).toBeTruthy();

    const openHubBtn = screen.getByRole('button', { name: /Open Facebook Hub/i });
    await fireEvent.click(openHubBtn);

    expect(navigated).toBe(true);
  });

  test('disconnects page and updates list after confirmation', async () => {
    render(FacebookAccountsSettings);

    const disconnectBtn = await screen.findByRole('button', { name: /Disconnect/i });
    await fireEvent.click(disconnectBtn);

    expect(await screen.findByText(/Disconnect Facebook Page/i)).toBeTruthy();
    expect(screen.getByText(/Are you sure you want to disconnect Page/i)).toBeTruthy();

    const confirmBtn = screen.getByRole('button', { name: /Confirm Disconnect/i });
    await fireEvent.click(confirmBtn);

    await waitFor(() => {
      expect(invokedCommands).toContainEqual({
        command: 'delete_facebook_account',
        args: { pageId: 'page_123' }
      });
    });

    expect(await screen.findByText('No Facebook Pages Connected')).toBeTruthy();
  });

  test('displays error message when listing fails', async () => {
    mockListError = new Error('Database connection locked');
    render(FacebookAccountsSettings);

    expect(await screen.findByText('Database connection locked')).toBeTruthy();
  });
});
