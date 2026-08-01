/// <reference types="bun-types" />
import { afterEach, beforeAll, describe, expect, test, mock } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import type {
  InboxItem,
  ConversationRuntimeStatus,
  ConversationMessage,
  TimelinePage,
  ConversationInvalidatedEvent
} from '$lib/services/conversationService';
import ConversationWorkspace from '../ConversationWorkspace.svelte';

beforeAll(() => {
  if (typeof globalThis.ResizeObserver === 'undefined') {
    globalThis.ResizeObserver = class ResizeObserver {
      observe() {}
      unobserve() {}
      disconnect() {}
    };
  }
});

let invalidationListeners = new Set<(event: ConversationInvalidatedEvent) => void>();

function emitInvalidated(event: ConversationInvalidatedEvent) {
  for (const listener of invalidationListeners) {
    listener(event);
  }
}

const mockCustomerItem: InboxItem = {
  key: 'lead:1',
  kind: 'customer',
  leadId: 1,
  conversationId: null,
  identityId: null,
  title: 'Nguyễn Văn A',
  avatarUrl: null,
  lastMessagePreview: 'Xin chào',
  lastMessageAt: '2026-07-31T10:00:00Z',
  channels: [
    { channel: 'zalo', messageCount: 5 },
    { channel: 'messenger', messageCount: 2 }
  ],
  replyTargets: [
    {
      conversationId: 10,
      channelAccountId: 1,
      channel: 'zalo',
      accountDisplayName: 'Zalo Main',
      conversationKind: 'direct',
      lastInboundAt: '2026-07-31T10:00:00Z',
      lastMessageAt: '2026-07-31T10:00:00Z'
    }
  ],
  participantCount: 1,
  participants: []
};

const mockGroupItem: InboxItem = {
  key: 'group-3',
  kind: 'group',
  leadId: null,
  conversationId: 30,
  identityId: null,
  title: 'Nhóm VIP',
  avatarUrl: null,
  lastMessagePreview: 'Thông báo',
  lastMessageAt: '2026-07-31T12:00:00Z',
  channels: [{ channel: 'zalo', messageCount: 12 }],
  replyTargets: [
    {
      conversationId: 30,
      channelAccountId: 1,
      channel: 'zalo',
      accountDisplayName: 'Zalo Main',
      conversationKind: 'group',
      lastInboundAt: '2026-07-31T12:00:00Z',
      lastMessageAt: '2026-07-31T12:00:00Z'
    }
  ],
  participantCount: 3,
  participants: [
    { identityId: 1, displayName: 'Minh', avatarUrl: null, participantRole: 'member' }
  ]
};

const mockMsg1: ConversationMessage = {
  id: 101,
  conversationId: 10,
  channelAccountId: 1,
  channel: 'zalo',
  accountDisplayName: 'Zalo Main',
  senderIdentityId: 100,
  senderDisplayName: 'Nguyễn Văn A',
  direction: 'inbound',
  providerMessageId: 'p-101',
  contentType: 'text',
  textContent: 'Chào shop',
  content: { text: 'Chào shop' },
  occurredAt: '2026-07-31T10:00:00Z',
  status: 'received',
  removedAt: null,
  errorMessage: null,
  replyToMessageId: null
};

const mockMsg2: ConversationMessage = {
  id: 102,
  conversationId: 10,
  channelAccountId: 2,
  channel: 'messenger',
  accountDisplayName: 'FB Page',
  senderIdentityId: 100,
  senderDisplayName: 'Nguyễn Văn A',
  direction: 'inbound',
  providerMessageId: 'p-102',
  contentType: 'text',
  textContent: 'Nhắn qua FB',
  content: { text: 'Nhắn qua FB' },
  occurredAt: '2026-07-31T10:05:00Z',
  status: 'received',
  removedAt: null,
  errorMessage: null,
  replyToMessageId: null
};

const mockListConversationInbox = mock(async (): Promise<InboxItem[]> => {
  return [mockCustomerItem, mockGroupItem];
});

const mockGetContactTimeline = mock(
  async (
    _leadId: number,
    _activeChannels: string[],
    _before: any,
    _limit: number
  ): Promise<TimelinePage> => {
    return {
      items: [mockMsg1, mockMsg2],
      nextCursor: { occurredAt: '2026-07-31T10:00:00Z', messageId: 101 }
    };
  }
);

const mockGetConversationTimeline = mock(
  async (_conversationId: number, _before: any, _limit: number): Promise<TimelinePage> => {
    return {
      items: [
        {
          id: 201,
          conversationId: 30,
          channelAccountId: 1,
          channel: 'zalo',
          accountDisplayName: 'Zalo Main',
          senderIdentityId: 1,
          senderDisplayName: 'Minh',
          direction: 'inbound',
          providerMessageId: 'p-201',
          contentType: 'text',
          textContent: 'Tin nhắn nhóm',
          content: { text: 'Tin nhắn nhóm' },
          occurredAt: '2026-07-31T12:00:00Z',
          status: 'received',
          removedAt: null,
          errorMessage: null,
          replyToMessageId: null
        }
      ],
      nextCursor: null
    };
  }
);

const mockGetConversationRuntimeStatus = mock(async (): Promise<ConversationRuntimeStatus> => {
  return {
    accounts: [
      {
        channelAccountId: 1,
        channel: 'zalo',
        providerAccountKey: 'zalo-1',
        displayName: 'Zalo Main',
        status: 'connected',
        lastConnectedAt: '2026-07-31T10:00:00Z',
        lastError: null
      }
    ]
  };
});

mock.module('$lib/services/conversationService', () => ({
  listConversationInbox: () => mockListConversationInbox(),
  getContactTimeline: (leadId: number, channels: any[], before: any, limit: number) =>
    mockGetContactTimeline(leadId, channels, before, limit),
  getConversationTimeline: (convId: number, before: any, limit: number) =>
    mockGetConversationTimeline(convId, before, limit),
  getConversationRuntimeStatus: () => mockGetConversationRuntimeStatus(),
  initConversationEventBridge: () => () => {},
  onConversationInvalidated: (listener: (event: ConversationInvalidatedEvent) => void) => {
    invalidationListeners.add(listener);
    return () => invalidationListeners.delete(listener);
  },
  sendConversationMessage: mock(async (req: any) => ({
    id: 999,
    conversationId: req.conversationId,
    channelAccountId: req.channelAccountId,
    channel: 'zalo',
    accountDisplayName: 'Zalo Main',
    senderIdentityId: 1,
    senderDisplayName: 'Agent',
    direction: 'outbound',
    providerMessageId: 'p-999',
    contentType: 'text',
    textContent: req.text,
    content: { text: req.text },
    occurredAt: '2026-07-31T13:00:00Z',
    status: 'queued',
    removedAt: null,
    errorMessage: null,
    replyToMessageId: null
  })),
  linkConversationIdentity: mock(async () => {})
}));

describe('ConversationWorkspace Component', () => {
  afterEach(() => {
    cleanup();
    document.body.innerHTML = '';
    invalidationListeners.clear();
    mockListConversationInbox.mockClear();
    mockGetContactTimeline.mockClear();
    mockGetConversationTimeline.mockClear();
    mockGetConversationRuntimeStatus.mockClear();
  });

  test('1. Mounting while activeTab !== "conversation" does not load inbox/runtime data', async () => {
    render(ConversationWorkspace, { props: { activeTab: 'crm' } });
    await tick();

    expect(mockListConversationInbox).not.toHaveBeenCalled();
    expect(mockGetConversationRuntimeStatus).not.toHaveBeenCalled();
  });

  test('2. First activation loads inbox and runtime exactly once', async () => {
    const { rerender } = render(ConversationWorkspace, { props: { activeTab: 'crm' } });
    await tick();

    expect(mockListConversationInbox).not.toHaveBeenCalled();

    await rerender({ activeTab: 'conversation' });
    await tick();
    await new Promise((r) => setTimeout(r, 20));

    expect(mockListConversationInbox).toHaveBeenCalledTimes(1);
    expect(mockGetConversationRuntimeStatus).toHaveBeenCalledTimes(1);
  });

  test('2b. Runtime polling runs only while Conversations is active', async () => {
    const { rerender } = render(ConversationWorkspace, {
      props: { activeTab: 'conversation', runtimeRefreshIntervalMs: 15 }
    });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 55));

    expect(mockGetConversationRuntimeStatus.mock.calls.length).toBeGreaterThan(1);

    await rerender({ activeTab: 'crm', runtimeRefreshIntervalMs: 15 });
    await tick();
    const callsWhileActive = mockGetConversationRuntimeStatus.mock.calls.length;
    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(mockGetConversationRuntimeStatus).toHaveBeenCalledTimes(callsWhileActive);
  });

  test('3. Selecting customer activates all available badges and calls getContactTimeline(leadId, activeChannels, null, 50)', async () => {
    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((r) => setTimeout(r, 20));

    expect(mockGetContactTimeline).toHaveBeenCalledWith(1, ['zalo', 'messenger'], null, 50);
  });

  test('4. Selecting group calls getConversationTimeline(conversationId, null, 50) without showing channel badges as filters', async () => {
    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((r) => setTimeout(r, 20));

    const groupRow = screen.getByText('Nhóm VIP');
    await fireEvent.click(groupRow);
    await tick();
    await new Promise((r) => setTimeout(r, 20));

    expect(mockGetConversationTimeline).toHaveBeenCalledWith(30, null, 50);
    expect(screen.queryByText('Không có kênh nào đang được hiển thị')).toBeNull();
  });

  test('5. Toggling badge filters loaded messages immediately and uses separate cache under channelSetKey', async () => {
    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((r) => setTimeout(r, 20));

    const messengerBadge = screen.getByRole('button', { name: 'messenger' });
    await fireEvent.click(messengerBadge);
    await tick();
    await new Promise((r) => setTimeout(r, 20));

    expect(mockGetContactTimeline).toHaveBeenCalledWith(1, ['zalo'], null, 50);
  });

  test('6. All badges inactive makes no timeline IPC call and shows explicit empty-filter state', async () => {
    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((r) => setTimeout(r, 20));

    const zaloBadge = screen.getByRole('button', { name: 'zalo' });
    const messengerBadge = screen.getByRole('button', { name: 'messenger' });

    await fireEvent.click(zaloBadge);
    await fireEvent.click(messengerBadge);
    await tick();

    mockGetContactTimeline.mockClear();

    await new Promise((r) => setTimeout(r, 20));

    const elements = screen.getAllByText('Không có kênh nào đang được hiển thị');
    expect(elements.length).toBeGreaterThanOrEqual(1);
    expect(mockGetContactTimeline).not.toHaveBeenCalled();
  });

  test('7. Older pagination uses current variant nextCursor and active channels', async () => {
    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((r) => setTimeout(r, 20));

    mockGetContactTimeline.mockClear();

    const timelineContainer = document.querySelector('[data-timeline-container]');
    expect(timelineContainer).toBeTruthy();

    await fireEvent.scroll(timelineContainer!, { target: { scrollTop: 0 } });
    await tick();
    await new Promise((r) => setTimeout(r, 20));

    expect(mockGetContactTimeline).toHaveBeenCalledWith(
      1,
      ['zalo', 'messenger'],
      { occurredAt: '2026-07-31T10:00:00Z', messageId: 101 },
      50
    );
  });

  test('8. Invalidation for open lead refreshes current variant; another lead marks inbox stale', async () => {
    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((r) => setTimeout(r, 20));

    mockGetContactTimeline.mockClear();
    mockListConversationInbox.mockClear();

    emitInvalidated({
      conversationId: 10,
      leadId: 1,
      channel: 'zalo',
      reason: 'message',
      messageId: 103
    });

    await new Promise((r) => setTimeout(r, 70));

    expect(mockGetContactTimeline).toHaveBeenCalledTimes(1);

    mockGetContactTimeline.mockClear();
    mockListConversationInbox.mockClear();

    emitInvalidated({
      conversationId: 99,
      leadId: 99,
      channel: 'zalo',
      reason: 'message',
      messageId: 104
    });

    await new Promise((r) => setTimeout(r, 70));

    expect(mockGetContactTimeline).not.toHaveBeenCalled();
    expect(mockListConversationInbox).toHaveBeenCalledTimes(1);
  });

  test('9. Link invalidation with conversationId = 0 reloads full inbox', async () => {
    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((r) => setTimeout(r, 20));

    mockListConversationInbox.mockClear();

    emitInvalidated({
      conversationId: 0,
      leadId: null,
      channel: 'zalo',
      reason: 'link'
    });

    await new Promise((r) => setTimeout(r, 70));

    expect(mockListConversationInbox).toHaveBeenCalledTimes(1);
  });

  test('10. Inactive workspace invalidations mark data stale and 1 refresh occurs on next activation', async () => {
    const { rerender } = render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((r) => setTimeout(r, 20));

    expect(mockListConversationInbox).toHaveBeenCalledTimes(1);

    await rerender({ activeTab: 'crm' });
    await tick();

    mockListConversationInbox.mockClear();
    mockGetContactTimeline.mockClear();

    emitInvalidated({
      conversationId: 10,
      leadId: 1,
      channel: 'zalo',
      reason: 'message'
    });

    await new Promise((r) => setTimeout(r, 70));

    expect(mockListConversationInbox).not.toHaveBeenCalled();
    expect(mockGetContactTimeline).not.toHaveBeenCalled();

    await rerender({ activeTab: 'conversation' });
    await tick();
    await new Promise((r) => setTimeout(r, 70));

    expect(mockListConversationInbox).toHaveBeenCalledTimes(1);
    expect(mockGetContactTimeline).toHaveBeenCalledTimes(1);
  });

  test('11. Queued outbound message appears immediately without waiting for realtime echo; later echo deduplicates by local row ID', async () => {
    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((r) => setTimeout(r, 20));

    const textarea = screen.getByPlaceholderText(/Nhập tin nhắn|Send message/i);
    await fireEvent.input(textarea, { target: { value: 'Tin nhắn gửi trực tiếp' } });

    const sendBtn = screen.getByRole('button', { name: /Gửi|Send/i });
    await fireEvent.click(sendBtn);
    await tick();

    const timelineContainer = document.querySelector('[data-timeline-container]');
    expect(timelineContainer?.textContent).toContain('Tin nhắn gửi trực tiếp');

    emitInvalidated({
      conversationId: 10,
      leadId: 1,
      channel: 'zalo',
      reason: 'message',
      messageId: 999
    });

    await new Promise((r) => setTimeout(r, 70));

    const bubbles = timelineContainer?.querySelectorAll('[data-message-id="999"]');
    expect(bubbles?.length).toBe(1);
  });

  test('12. Renders LinkedLeadActions when selectedItem.kind === "customer"', async () => {
    const mockLeadItem: InboxItem = {
      ...mockCustomerItem,
      key: 'lead:1',
      kind: 'customer'
    };
    mockListConversationInbox.mockImplementationOnce(async () => [mockLeadItem]);

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((r) => setTimeout(r, 20));

    expect(screen.getByRole('button', { name: /Đổi lead/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /Hủy liên kết/i })).toBeTruthy();
  });
});
