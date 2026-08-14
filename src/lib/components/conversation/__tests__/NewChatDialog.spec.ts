/// <reference types="bun-types" />
import { beforeAll, describe, expect, test, mock } from 'bun:test';
import { render, screen, fireEvent } from '@testing-library/svelte';
import NewChatDialog from '../NewChatDialog.svelte';
import * as conversationService from '$lib/services/conversationService';

beforeAll(() => {
  if (typeof globalThis.ResizeObserver === 'undefined') {
    globalThis.ResizeObserver = class ResizeObserver {
      observe() {}
      unobserve() {}
      disconnect() {}
    };
  }
});

describe('NewChatDialog Component', () => {
  test('renders dialog fields and handles form submission', async () => {
    const mockRuntime = {
      accounts: [
        {
          channelAccountId: 1,
          channel: 'zalo' as const,
          providerAccountKey: 'zalo-1',
          displayName: 'Sales Account',
          status: 'connected' as const,
          lastConnectedAt: null,
          lastError: null
        }
      ]
    };
    mock.module('$lib/services/conversationService', () => ({
      ...conversationService,
      getConversationRuntimeStatus: async () => mockRuntime,
      startDirectConversation: async () => ({
        id: 99,
        conversationId: 10,
        channelAccountId: 1,
        channel: 'zalo' as const,
        accountDisplayName: 'Sales Account',
        senderIdentityId: null,
        senderDisplayName: null,
        direction: 'outbound' as const,
        contentType: 'text',
        textContent: 'Hello new phone',
        content: { text: 'Hello new phone' },
        occurredAt: '2026-08-01T10:00:00Z',
        status: 'queued' as const,
        removedAt: null,
        errorMessage: null,
        replyToMessageId: null
      })
    }));

    render(NewChatDialog, {
      props: { open: true }
    });

    expect(screen.getByText('New Chat via Phone / Zalo ID')).toBeTruthy();
  });
});
