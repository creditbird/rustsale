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
  if (typeof document !== 'undefined') {
    document.body.innerHTML = '';
    document.body.removeAttribute('style');
  }
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

    const advBtn = screen.getByRole('button', { name: /Advanced Settings/i });
    await fireEvent.click(advBtn);

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
    expect(screen.getByText(/Discovered Fanpages/i)).toBeTruthy();

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

    const advBtn = screen.getByRole('button', { name: /Advanced Settings/i });
    await fireEvent.click(advBtn);

    const appIdInput = screen.getByPlaceholderText(/Enter App ID/i);
    await fireEvent.input(appIdInput, { target: { value: '1075678647572774' } });

    const copyBtn = screen.getByRole('button', { name: /Copy Link/i });
    await fireEvent.click(copyBtn);

    expect(clipboardText).toMatch(/^https:\/\/www\.facebook\.com\/v26\.0\/dialog\/oauth\?client_id=1075678647572774/);
    expect(clipboardText).toMatch(/state=([^&]+)/);
    expect(await screen.findByText('Copied')).toBeTruthy();
  });

  test('validates and prompts for input when Access Token is empty in OAuth tab', async () => {
    render(FacebookAccountsSettings);

    await fireEvent.click(await screen.findByRole('button', { name: /Connect Fanpage/i }));

    expect(await screen.findByText(/Connect Facebook Fanpage/i)).toBeTruthy();

    const fetchBtn = screen.getByRole('button', { name: /Fetch Pages/i });
    await fireEvent.click(fetchBtn);

    expect(await screen.findByText(/Please enter or paste an Access Token/i)).toBeTruthy();
  });

  test('copies Supabase OAuth callback redirect URI to clipboard', async () => {
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

    expect(await screen.findByText(/Connect Facebook Fanpage/i)).toBeTruthy();

    const advBtn = screen.getByRole('button', { name: /Advanced Settings/i });
    await fireEvent.click(advBtn);

    const copyCallbackBtn = screen.getByRole('button', { name: /Copy Callback URL/i });
    await fireEvent.click(copyCallbackBtn);

    expect(clipboardText).toContain('/auth/v1/callback');
    expect(await screen.findByText(/Copied/i)).toBeTruthy();
  });

  test('extracts provider_token from Supabase OAuth redirect URL', async () => {
    mockDiscoveredPages = [
      {
        id: 'page_supabase_1',
        name: 'Supabase Connected Page',
        category: 'Business',
        avatarUrl: null,
        accessToken: 'EAAB_provider_token_123',
        permissions: ['pages_messaging', 'pages_read_engagement']
      }
    ];

    render(FacebookAccountsSettings);

    expect(await screen.findByText('RustSale Store')).toBeTruthy();

    await fireEvent.click(await screen.findByRole('button', { name: /Connect Fanpage/i }));

    expect(await screen.findByText(/Connect Facebook Fanpage/i)).toBeTruthy();

    let capturedAuthUrl = '';
    window.open = (url?: string | URL) => {
      capturedAuthUrl = String(url);
      return null;
    };

    const openMetaBtn = await screen.findByRole('button', { name: /Open Meta Login/i });
    await fireEvent.click(openMetaBtn);

    await waitFor(() => {
      const openerCmd = invokedCommands.find((c) => c.command.includes('opener'));
      const url = openerCmd?.args?.path || openerCmd?.args?.url || capturedAuthUrl;
      expect(url).toMatch(/state=([^&]+)/);
    });

    const openerCmd = invokedCommands.find((c) => c.command.includes('opener'));
    const authUrl = String(openerCmd?.args?.path || openerCmd?.args?.url || capturedAuthUrl);
    const stateMatch = authUrl.match(/state=([^&]+)/);
    expect(stateMatch).toBeTruthy();

    const tokenInput = screen.getByPlaceholderText(/Paste Access Token/i);
    await fireEvent.input(tokenInput, {
      target: {
        value: `https://xyzcompany.supabase.co/auth/v1/callback#provider_token=EAAB_provider_token_123&state=${stateMatch![1]}`
      }
    });

    const fetchBtn = screen.getByRole('button', { name: /Fetch Pages/i });
    await fireEvent.click(fetchBtn);

    expect(await screen.findByText('Supabase Connected Page')).toBeTruthy();
  });

  test('rejects unsafe OAuth redirect URIs such as javascript: or non-HTTPS schemes', async () => {
    render(FacebookAccountsSettings);

    expect(await screen.findByText('RustSale Store')).toBeTruthy();

    await fireEvent.click(await screen.findByRole('button', { name: /Connect Fanpage/i }));

    expect(await screen.findByText(/Connect Facebook Fanpage/i)).toBeTruthy();

    const advBtn = screen.getByRole('button', { name: /Advanced Settings/i });
    await fireEvent.click(advBtn);

    const redirectInput = screen.getByPlaceholderText(/https:\/\/<project-ref>\.supabase\.co/i);
    await fireEvent.input(redirectInput, { target: { value: 'javascript:alert(1)' } });

    const openMetaBtn = await screen.findByRole('button', { name: /Open Meta Login/i });
    await fireEvent.click(openMetaBtn);

    expect(await screen.findByText(/Invalid OAuth Redirect URI/i)).toBeTruthy();
  });

  test('ignores invalid previous redirect URI stored in localStorage and falls back to safe default', async () => {
    localStorage.setItem('rustsale_facebook_redirect_uri', 'javascript:evil()');

    render(FacebookAccountsSettings);

    expect(await screen.findByText('RustSale Store')).toBeTruthy();

    await fireEvent.click(await screen.findByRole('button', { name: /Connect Fanpage/i }));

    expect(await screen.findByText(/Connect Facebook Fanpage/i)).toBeTruthy();

    let capturedAuthUrl = '';
    window.open = (url?: string | URL) => {
      capturedAuthUrl = String(url);
      return null;
    };

    const openMetaBtn = await screen.findByRole('button', { name: /Open Meta Login/i });
    await fireEvent.click(openMetaBtn);

    await waitFor(() => {
      const openerCmd = invokedCommands.find((c) => c.command.includes('opener'));
      const url = openerCmd?.args?.path || openerCmd?.args?.url || capturedAuthUrl;
      expect(url).toMatch(/redirect_uri=https%3A%2F%2F/);
      expect(url).not.toMatch(/javascript/);
    });
  });

  test('rejects provider_token when callback origin is invalid or unsafe', async () => {
    render(FacebookAccountsSettings);

    expect(await screen.findByText('RustSale Store')).toBeTruthy();

    await fireEvent.click(await screen.findByRole('button', { name: /Connect Fanpage/i }));

    let capturedAuthUrl = '';
    window.open = (url?: string | URL) => {
      capturedAuthUrl = String(url);
      return null;
    };

    const openMetaBtn = await screen.findByRole('button', { name: /Open Meta Login/i });
    await fireEvent.click(openMetaBtn);

    await waitFor(() => {
      const openerCmd = invokedCommands.find((c) => c.command.includes('opener'));
      const url = openerCmd?.args?.path || openerCmd?.args?.url || capturedAuthUrl;
      expect(url).toMatch(/state=([^&]+)/);
    });

    const openerCmd = invokedCommands.find((c) => c.command.includes('opener'));
    const authUrl = String(openerCmd?.args?.path || openerCmd?.args?.url || capturedAuthUrl);
    const stateMatch = authUrl.match(/state=([^&]+)/);

    const tokenInput = screen.getByPlaceholderText(/Paste Access Token/i);
    await fireEvent.input(tokenInput, {
      target: {
        value: `http://insecure-domain.xyz/auth/v1/callback#provider_token=EAAB_fake&state=${stateMatch![1]}`
      }
    });

    const fetchBtn = screen.getByRole('button', { name: /Fetch Pages/i });
    await fireEvent.click(fetchBtn);

    expect(await screen.findByText(/not recognized as a valid secure domain/i)).toBeTruthy();
  });

  test('rejects domain confusion attack URLs (e.g. rustsale.trantrongtri.com.attacker.com)', async () => {
    render(FacebookAccountsSettings);

    expect(await screen.findByText('RustSale Store')).toBeTruthy();

    await fireEvent.click(await screen.findByRole('button', { name: /Connect Fanpage/i }));

    expect(await screen.findByText(/Connect Facebook Fanpage/i)).toBeTruthy();

    const advBtn = screen.getByRole('button', { name: /Advanced Settings/i });
    await fireEvent.click(advBtn);

    const redirectInput = screen.getByPlaceholderText(/https:\/\/<project-ref>\.supabase\.co/i);
    await fireEvent.input(redirectInput, { target: { value: 'https://rustsale.trantrongtri.com.attacker.com/oauth/callback' } });

    const openMetaBtn = await screen.findByRole('button', { name: /Open Meta Login/i });
    await fireEvent.click(openMetaBtn);

    expect(await screen.findByText(/Invalid OAuth Redirect URI/i)).toBeTruthy();
  });

  test('validates approved hostnames via isAllowedOAuthHost and isValidOAuthRedirectUri', async () => {
    const { isAllowedOAuthHost, isValidOAuthRedirectUri } = await import('../FacebookAccountsSettings.svelte');
    
    // Approved hosts
    expect(isAllowedOAuthHost('rustsale.trantrongtri.com')).toBe(true);
    expect(isAllowedOAuthHost('trantrongtri.com')).toBe(true);
    expect(isAllowedOAuthHost('xyz.supabase.co')).toBe(true);
    expect(isAllowedOAuthHost('supabase.co')).toBe(true);
    expect(isAllowedOAuthHost('localhost')).toBe(true);
    expect(isAllowedOAuthHost('127.0.0.1')).toBe(true);
    expect(isAllowedOAuthHost('mycustombackend.org', 'https://mycustombackend.org/auth/v1/callback')).toBe(true);

    // Rejected hosts (domain confusion & arbitrary external)
    expect(isAllowedOAuthHost('rustsale.trantrongtri.com.attacker.com')).toBe(false);
    expect(isAllowedOAuthHost('supabase.co.attacker.com')).toBe(false);
    expect(isAllowedOAuthHost('attacker.example.com')).toBe(false);
    expect(isAllowedOAuthHost('eviltrantrongtri.com')).toBe(false);

    // Full URI validation
    expect(isValidOAuthRedirectUri('https://xyz.supabase.co/auth/v1/callback')).toBe(true);
    expect(isValidOAuthRedirectUri('https://rustsale.trantrongtri.com/oauth/callback.html')).toBe(true);
    expect(isValidOAuthRedirectUri('http://localhost:5173/auth/callback')).toBe(true);
    expect(isValidOAuthRedirectUri('javascript:alert(1)')).toBe(false);
    expect(isValidOAuthRedirectUri('file:///etc/passwd')).toBe(false);
    expect(isValidOAuthRedirectUri('https://attacker.example.com/oauth/callback')).toBe(false);
    expect(isValidOAuthRedirectUri('https://rustsale.trantrongtri.com.evil.com/callback')).toBe(false);
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

  test('extracts token from OAuth callback even if state was omitted by intermediary redirect', async () => {
    mockDiscoveredPages = [
      {
        id: 'page_redirect_1',
        name: 'Redirect Connected Page',
        category: 'Business',
        avatarUrl: null,
        accessToken: 'EAAWh6_test_token',
        permissions: ['pages_messaging', 'pages_read_engagement']
      }
    ];

    render(FacebookAccountsSettings);

    expect(await screen.findByText('RustSale Store')).toBeTruthy();

    await fireEvent.click(await screen.findByRole('button', { name: /Connect Fanpage/i }));

    const openMetaBtn = await screen.findByRole('button', { name: /Open Meta Login/i });
    await fireEvent.click(openMetaBtn);

    const tokenInput = screen.getByPlaceholderText(/Paste Access Token/i);
    await fireEvent.input(tokenInput, {
      target: {
        value: 'http://localhost:3000/?error=invalid_request&error_code=bad_oauth_callback&error_description=OAuth+state+parameter+missing#access_token=EAAWh6_test_token'
      }
    });

    const fetchBtn = screen.getByRole('button', { name: /Fetch Pages/i });
    await fireEvent.click(fetchBtn);

    expect(await screen.findByText('Redirect Connected Page')).toBeTruthy();
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
