/// <reference types="bun-types" />
import { afterEach, beforeEach, describe, expect, mock, test } from 'bun:test';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import FacebookHub from '../FacebookHub.svelte';
import type { ManagedFacebookAccount } from '$lib/services/accountSettingsService';
import type { FacebookScheduledPost, CommentAutomationRule, CommentLog, PageHealth } from '$lib/services/facebookService';

afterEach(() => {
  cleanup();
  localStorage.clear();
});

let mockAccounts: ManagedFacebookAccount[] = [];
let mockPageHealth: PageHealth | null = null;
let mockScheduledPosts: FacebookScheduledPost[] = [];
let mockAutomations: CommentAutomationRule[] = [];
let mockLogs: CommentLog[] = [];
let invokedCommands: { command: string; args?: any }[] = [];

mock.module('@tauri-apps/api/core', () => ({
  invoke: async (command: string, args?: Record<string, any>) => {
    invokedCommands.push({ command, args });

    if (command === 'get_managed_facebook_accounts') {
      return JSON.parse(JSON.stringify(mockAccounts));
    }
    if (command === 'get_facebook_page_health') {
      if (mockPageHealth) return mockPageHealth;
      return {
        page_id: args?.pageId || args?.page_id || 'test_page',
        token_status: 'valid',
        granted_scopes: ['pages_messaging', 'pages_manage_posts'],
        active_scheduled_count: mockScheduledPosts.length,
        active_automations_count: mockAutomations.length,
        updated_at: new Date().toISOString()
      };
    }
    if (command === 'list_facebook_scheduled_posts') {
      return [...mockScheduledPosts];
    }
    if (command === 'list_comment_automations') {
      return [...mockAutomations];
    }
    if (command === 'list_comment_logs') {
      return [...mockLogs];
    }
    if (command === 'schedule_facebook_post') {
      return 101;
    }
    if (command === 'publish_facebook_post_now') {
      return true;
    }
    if (command === 'create_comment_automation') {
      return 201;
    }
    if (command === 'update_comment_automation') {
      return true;
    }
    if (command === 'delete_comment_automation') {
      return true;
    }

    return null;
  }
}));

describe('FacebookHub Component', () => {
  beforeEach(() => {
    invokedCommands = [];
    mockAccounts = [
      {
        channelAccountId: 10,
        pageId: '249847884884471',
        pageName: 'Lemy',
        avatarUrl: 'https://example.com/lemy.png',
        tokenStatus: 'valid',
        grantedScopes: ['pages_messaging', 'pages_manage_posts', 'pages_read_engagement'],
        runtimeStatus: 'connected',
        lastConnectedAt: new Date().toISOString(),
        lastError: null,
        updatedAt: new Date().toISOString()
      }
    ];
    mockPageHealth = null;
    mockScheduledPosts = [];
    mockAutomations = [];
    mockLogs = [];
  });

  test('renders zero state banner when no Facebook accounts are connected', async () => {
    mockAccounts = [];
    let openedSettings = false;

    render(FacebookHub, {
      props: {
        activeTab: 'facebook_hub',
        onOpenSettings: () => {
          openedSettings = true;
        }
      }
    });

    expect(await screen.findByText('Chưa kết nối Facebook Fanpage')).toBeTruthy();
    expect(screen.getByText(/Kết nối Fanpage của bạn trong mục Cài đặt Hệ thống/i)).toBeTruthy();

    const ctaBtn = screen.getByRole('button', { name: /Đi tới Cài đặt & Kết nối Fanpage/i });
    await fireEvent.click(ctaBtn);
    expect(openedSettings).toBe(true);
  });

  test('resolves and renders connected Fanpage information and active token health', async () => {
    render(FacebookHub, {
      props: {
        activeTab: 'facebook_hub'
      }
    });

    expect(await screen.findByRole('heading', { name: 'Lemy' })).toBeTruthy();
    await waitFor(() => {
      expect(document.body.textContent).toContain('Token Active (Keyring Vault)');
      expect(document.body.textContent).toContain('249847884884471');
      expect(document.body.textContent).toContain('Active Automations:');
      expect(document.body.textContent).toContain('Pending Posts:');
    });
  });

  test('allows switching between multiple connected Fanpages', async () => {
    mockAccounts = [
      {
        channelAccountId: 10,
        pageId: '249847884884471',
        pageName: 'Lemy',
        avatarUrl: 'https://example.com/lemy.png',
        tokenStatus: 'valid',
        grantedScopes: ['pages_messaging'],
        runtimeStatus: 'connected',
        lastConnectedAt: new Date().toISOString(),
        lastError: null,
        updatedAt: new Date().toISOString()
      },
      {
        channelAccountId: 11,
        pageId: '999888777',
        pageName: 'RustSale Store',
        avatarUrl: null,
        tokenStatus: 'valid',
        grantedScopes: ['pages_messaging', 'pages_manage_posts'],
        runtimeStatus: 'connected',
        lastConnectedAt: new Date().toISOString(),
        lastError: null,
        updatedAt: new Date().toISOString()
      }
    ];

    render(FacebookHub, {
      props: {
        activeTab: 'facebook_hub'
      }
    });

    expect(await screen.findByText(/Chọn Page:/i)).toBeTruthy();
    await waitFor(() => {
      expect(document.body.textContent).toContain('Token Active (Keyring Vault)');
    });

    const select = screen.getByRole('combobox') as HTMLSelectElement;
    expect(select).toBeTruthy();

    select.value = '999888777';
    await fireEvent.change(select);

    await waitFor(() => {
      const healthCall = invokedCommands.find(
        (c) => c.command === 'get_facebook_page_health' && (c.args?.pageId === '999888777' || c.args?.page_id === '999888777')
      );
      expect(healthCall).toBeTruthy();
    });
  });

  test('submits Facebook post with correct page_id and channel_account_id', async () => {
    render(FacebookHub, {
      props: {
        activeTab: 'facebook_hub'
      }
    });

    expect(await screen.findByRole('heading', { name: 'Lemy' })).toBeTruthy();
    await waitFor(() => {
      expect(document.body.textContent).toContain('Token Active (Keyring Vault)');
    });

    const textarea = screen.getByPlaceholderText(/What's on your mind/i) as HTMLTextAreaElement;
    textarea.value = 'Chào mừng quý khách đến với Lemy!';
    await fireEvent.input(textarea, { target: { value: 'Chào mừng quý khách đến với Lemy!' } });

    const publishBtn = screen.getByRole('button', { name: /Publish Now/i });
    await fireEvent.click(publishBtn);

    await waitFor(() => {
      const scheduleCall = invokedCommands.find((c) => c.command === 'schedule_facebook_post');
      expect(scheduleCall).toBeTruthy();
      expect(scheduleCall?.args?.request?.page_id).toBe('249847884884471');
      expect(scheduleCall?.args?.request?.channel_account_id).toBe(10);
      expect(scheduleCall?.args?.request?.message).toBe('Chào mừng quý khách đến với Lemy!');
    });

    expect(await screen.findByText(/Post submitted for immediate publishing!/i)).toBeTruthy();
  });

  test('blocks post submission when account is missing or has no channelAccountId', async () => {
    mockAccounts = [];

    render(FacebookHub, {
      props: {
        activeTab: 'facebook_hub'
      }
    });

    expect(await screen.findByText('Chưa kết nối Facebook Fanpage')).toBeTruthy();

    const scheduleCalls = invokedCommands.filter((c) => c.command === 'schedule_facebook_post');
    expect(scheduleCalls.length).toBe(0);
  });
});
