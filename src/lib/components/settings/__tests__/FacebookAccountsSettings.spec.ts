import { afterEach, beforeEach, describe, expect, mock, test } from 'bun:test';
import { setLocale } from '$lib/i18n';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import FacebookAccountsSettings from '../FacebookAccountsSettings.svelte';
import type {
  DiscoveredFacebookPage,
  ManagedFacebookAccount,
  SaveFacebookAccountPayload
} from '$lib/services/accountSettingsService';

let invokedCommands: Array<{ command: string; args?: Record<string, any> }> = [];
let mockAccounts: ManagedFacebookAccount[] = [];
let mockDiscoveredPages: DiscoveredFacebookPage[] = [];
let mockListError: Error | null = null;
let mockDeleteError: Error | null = null;
let mockFetchError: Error | null = null;
let mockSaveError: Error | null = null;

mock.module('@tauri-apps/api/core', () => ({
  invoke: async (command: string, args?: Record<string, any>) => {
    invokedCommands.push({ command, args });
    switch (command) {
      case 'get_managed_facebook_accounts':
        if (mockListError) throw mockListError;
        return JSON.parse(JSON.stringify(mockAccounts));
      case 'delete_facebook_account':
        if (mockDeleteError) throw mockDeleteError;
        mockAccounts = mockAccounts.filter((a) => a.pageId !== args?.pageId);
        return undefined;
      case 'fetch_facebook_pages_from_token':
        if (mockFetchError) throw mockFetchError;
        return JSON.parse(JSON.stringify(mockDiscoveredPages));
      case 'save_managed_facebook_account': {
        if (mockSaveError) throw mockSaveError;
        const req = args?.request as SaveFacebookAccountPayload;
        const newAcc: ManagedFacebookAccount = {
          channelAccountId: 200,
          pageId: req.pageId,
          pageName: req.pageName || req.pageId,
          avatarUrl: req.avatarUrl || null,
          tokenStatus: 'valid',
          grantedScopes: req.grantedScopes || ['pages_messaging'],
          runtimeStatus: 'connected',
          lastConnectedAt: new Date().toISOString(),
          lastError: null,
          updatedAt: new Date().toISOString()
        };
        mockAccounts.push(newAcc);
        return newAcc;
      }
      default:
        return undefined;
    }
  }
}));

beforeEach(() => {
  setLocale('en');
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem('rustsale_meta_app_id', '1075678647572774');
  }
  invokedCommands = [];
  mockListError = null;
  mockDeleteError = null;
  mockFetchError = null;
  mockSaveError = null;
  mockDiscoveredPages = [];
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
  mockListError = null;
  mockDeleteError = null;
  mockFetchError = null;
  mockSaveError = null;
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

  test('connects via OAuth token discovery and saves selected pages', async () => {
    mockDiscoveredPages = [
      {
        id: 'page_discovered_999',
        name: 'Discovered Coffee Page',
        category: 'Coffee Shop',
        avatarUrl: 'https://example.com/coffee.png',
        accessToken: 'EAAB_token_page_999',
        permissions: ['pages_messaging', 'pages_manage_posts']
      }
    ];

    render(FacebookAccountsSettings);

    await fireEvent.click(await screen.findByRole('button', { name: /Connect Fanpage/i }));

    expect(await screen.findByText(/Connect Facebook Fanpage/i)).toBeTruthy();
    expect(screen.getByText(/Login with Facebook \(OAuth\)/i)).toBeTruthy();

    let capturedAuthUrl = '';
    window.open = (url?: string | URL) => {
      capturedAuthUrl = String(url);
      return null;
    };

    const appIdInput = screen.getByPlaceholderText(/Enter App ID/i);
    await fireEvent.input(appIdInput, { target: { value: '1075678647572774' } });

    const openLoginBtn = screen.getByRole('button', { name: /Open Meta Login/i });
    await fireEvent.click(openLoginBtn);

    await waitFor(() => {
      const openerCmd = invokedCommands.find((c) => c.command.includes('opener'));
      const url = openerCmd?.args?.path || openerCmd?.args?.url || capturedAuthUrl;
      expect(url).toMatch(/client_id=1075678647572774/);
      expect(url).toMatch(/state=([^&]+)/);
    });

    const openerCmd = invokedCommands.find((c) => c.command.includes('opener'));
    const authUrl = String(openerCmd?.args?.path || openerCmd?.args?.url || capturedAuthUrl);
    const stateMatch = authUrl.match(/state=([^&]+)/);
    expect(stateMatch).toBeTruthy();

    const tokenInput = screen.getByPlaceholderText(/Paste Access Token/i);
    await fireEvent.input(tokenInput, {
      target: { value: `https://facebook.com/connect/login_success.html#access_token=EAAB_user_token_abc&state=${stateMatch![1]}` }
    });

    const fetchBtn = screen.getByRole('button', { name: /Fetch Pages/i });
    await fireEvent.click(fetchBtn);

    expect(await screen.findByText('Discovered Coffee Page')).toBeTruthy();
    expect(screen.getByText(/Found 1 Fanpage\(s\)/i)).toBeTruthy();

    const connectSelectedBtn = screen.getByRole('button', { name: /Connect Selected Pages/i });
    await fireEvent.click(connectSelectedBtn);

    await waitFor(() => {
      expect(invokedCommands).toContainEqual({
        command: 'save_managed_facebook_account',
        args: {
          request: {
            pageId: 'page_discovered_999',
            pageName: 'Discovered Coffee Page',
            avatarUrl: 'https://example.com/coffee.png',
            accessToken: 'EAAB_token_page_999',
            grantedScopes: ['pages_messaging', 'pages_manage_posts']
          }
        }
      });
    });

    await waitFor(() => {
      expect(screen.getByText(/page_discovered_999/)).toBeTruthy();
    });
  });

  test('copies OAuth authorization link to clipboard', async () => {
    let clipboardText = '';
    Object.assign(navigator, {
      clipboard: {
        writeText: async (text: string) => {
          clipboardText = text;
        }
      }
    });

    render(FacebookAccountsSettings);

    await fireEvent.click(await screen.findByRole('button', { name: /Connect Fanpage/i }));

    const appIdInput = screen.getByPlaceholderText(/Enter App ID/i);
    await fireEvent.input(appIdInput, { target: { value: '1075678647572774' } });

    const copyBtn = screen.getByRole('button', { name: /Copy Link/i });
    await fireEvent.click(copyBtn);

    expect(clipboardText).toMatch(/^https:\/\/www\.facebook\.com\/v26\.0\/dialog\/oauth\?client_id=1075678647572774/);
    expect(clipboardText).toMatch(/state=([^&]+)/);
    expect(await screen.findByText('Copied')).toBeTruthy();
  });

  test('validates and prompts for Meta App ID when App ID is empty in OAuth tab', async () => {
    render(FacebookAccountsSettings);

    await fireEvent.click(await screen.findByRole('button', { name: /Connect Fanpage/i }));

    const appIdInput = screen.getByPlaceholderText(/Enter App ID/i);
    await fireEvent.input(appIdInput, { target: { value: '' } });

    const openLoginBtn = screen.getByRole('button', { name: /Open Meta Login/i });
    await fireEvent.click(openLoginBtn);

    expect(await screen.findByText(/Please enter your Meta App ID above/i)).toBeTruthy();
  });

  test('connects via manual tab with Page ID and Access Token', async () => {
    render(FacebookAccountsSettings);

    await fireEvent.click(await screen.findByRole('button', { name: /Connect Fanpage/i }));

    const manualTab = await screen.findByRole('button', { name: /Manual Access Token/i });
    await fireEvent.click(manualTab);

    const pageIdInput = screen.getByPlaceholderText('e.g. 102938475610293');
    const pageNameInput = screen.getByPlaceholderText('e.g. RustSale Support Fanpage');
    const tokenInput = screen.getByPlaceholderText('EAAB...');

    await fireEvent.input(pageIdInput, { target: { value: 'page_manual_777' } });
    await fireEvent.input(pageNameInput, { target: { value: 'Manual Store Page' } });
    await fireEvent.input(tokenInput, { target: { value: 'EAAB_manual_secret' } });

    const connectBtn = screen.getByRole('button', { name: /Save & Connect Page/i });
    await fireEvent.click(connectBtn);

    await waitFor(() => {
      expect(invokedCommands).toContainEqual({
        command: 'save_managed_facebook_account',
        args: {
          request: {
            pageId: 'page_manual_777',
            pageName: 'Manual Store Page',
            avatarUrl: null,
            accessToken: 'EAAB_manual_secret',
            grantedScopes: [
              'pages_show_list',
              'pages_read_engagement',
              'pages_manage_posts',
              'pages_messaging'
            ]
          }
        }
      });
    });

    expect(await screen.findByText('Manual Store Page')).toBeTruthy();
  });

  test('rejects callback with missing OAuth state parameter when OAuth was initiated', async () => {
    render(FacebookAccountsSettings);

    await fireEvent.click(await screen.findByRole('button', { name: /Connect Fanpage/i }));

    const openMetaBtn = await screen.findByRole('button', { name: /Open Meta Login/i });
    await fireEvent.click(openMetaBtn);

    const tokenInput = screen.getByPlaceholderText(/Paste Access Token/i);
    await fireEvent.input(tokenInput, {
      target: { value: 'https://facebook.com/connect/login_success.html#access_token=EAAB_test_without_state' }
    });

    const fetchBtn = screen.getByRole('button', { name: /Fetch Pages/i });
    await fireEvent.click(fetchBtn);

    expect(await screen.findByText(/missing required state parameter/i)).toBeTruthy();
  });

  test('rejects callback with mismatched OAuth state nonce', async () => {
    render(FacebookAccountsSettings);

    await fireEvent.click(await screen.findByRole('button', { name: /Connect Fanpage/i }));

    const openMetaBtn = await screen.findByRole('button', { name: /Open Meta Login/i });
    await fireEvent.click(openMetaBtn);

    const tokenInput = screen.getByPlaceholderText(/Paste Access Token/i);
    await fireEvent.input(tokenInput, {
      target: { value: 'https://facebook.com/connect/login_success.html#access_token=EAAB_evil&state=wrong_nonce' }
    });

    const fetchBtn = screen.getByRole('button', { name: /Fetch Pages/i });
    await fireEvent.click(fetchBtn);

    expect(await screen.findByText(/OAuth state nonce mismatch/i)).toBeTruthy();
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
    mockListError = null;
  });
});
