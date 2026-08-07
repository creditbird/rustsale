/// <reference types="bun-types" />
import { afterEach, beforeAll, describe, expect, test, mock } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import type {
  InboxItem,
  ConversationRuntimeStatus,
  ConversationMessage,
  TimelinePage,
  ConversationInvalidatedEvent,
  ConversationCapabilities,
  SendConversationBatchRequest,
  SetConversationReactionRequest
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

const richCapabilities: ConversationCapabilities = {
  maxTextLength: 2000,
  maxAttachmentsPerBatch: 10,
  maxFileSizeBytes: 50 * 1024 * 1024,
  supportedImageTypes: ['image/png'],
  supportedVideoTypes: ['video/mp4'],
  supportedAudioTypes: ['audio/mpeg'],
  supportedDocTypes: ['application/pdf'],
  supportsStickers: true,
  supportsQuoteReply: true,
  supportsRecall: true,
  recallWindowSeconds: 86400,
  supportsReactions: true,
  reactions: {
    read: true,
    write: true,
    supported: ['heart', 'like', 'haha', 'wow', 'cry', 'angry']
  },
  forward: { enabled: true, maxTargets: 10 },
  mentions: { enabled: false, maxPerMessage: 0 },
  quickTemplates: { enabled: false, maxTemplates: 0 }
};

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

const mockUnknownItem: InboxItem = {
  key: 'identity:28',
  kind: 'unknown',
  leadId: null,
  conversationId: null,
  identityId: 28,
  title: 'Hồ Ngọc Phương Thảo',
  avatarUrl: null,
  lastMessagePreview: 'Chồng iu ơi',
  lastMessageAt: '2026-08-01T03:16:38Z',
  channels: [{ channel: 'zalo', messageCount: 6 }],
  replyTargets: [
    {
      conversationId: 28,
      channelAccountId: 1,
      channel: 'zalo',
      accountDisplayName: 'Hello World',
      conversationKind: 'direct',
      lastInboundAt: '2026-08-01T03:14:08Z',
      lastMessageAt: '2026-08-01T03:16:38Z'
    }
  ],
  participantCount: 0,
  participants: []
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
  contentType: 'text',
  textContent: 'Chào shop',
  content: { text: 'Chào shop' },
  occurredAt: '2026-07-31T10:00:00Z',
  status: 'received',
  removedAt: null,
  errorMessage: null,
  replyToMessageId: null,
  attachments: [],
  replyTarget: null,
  reactions: [],
  batchIndex: null,
  sendBatch: null,
  actions: { reply: true, copy: true, recall: false, forward: true, react: true }
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
  contentType: 'text',
  textContent: 'Nhắn qua FB',
  content: { text: 'Nhắn qua FB' },
  occurredAt: '2026-07-31T10:05:00Z',
  status: 'received',
  removedAt: null,
  errorMessage: null,
  replyToMessageId: null,
  attachments: [],
  replyTarget: null,
  reactions: [],
  batchIndex: null,
  sendBatch: null
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
          contentType: 'text',
          textContent: 'Tin nhắn nhóm',
          content: { text: 'Tin nhắn nhóm' },
          occurredAt: '2026-07-31T12:00:00Z',
          status: 'received',
          removedAt: null,
          errorMessage: null,
          replyToMessageId: null,
          attachments: [],
          replyTarget: null,
          reactions: [],
          batchIndex: null,
          sendBatch: null,
          actions: { reply: true, copy: true, recall: false, forward: true, react: true }
        }
      ],
      nextCursor: null
    };
  }
);

const mockGetIdentityTimeline = mock(
  async (_identityId: number, _before: any, _limit: number): Promise<TimelinePage> => {
    return {
      items: [mockMsg1],
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
        capabilities: richCapabilities,
        status: 'connected',
        lastConnectedAt: '2026-07-31T10:00:00Z',
        lastError: null
      }
    ]
  };
});

const mockSendConversationBatch = mock(async (request: SendConversationBatchRequest) => ({
  batchId: 'batch-optimistic',
  clientBatchId: request.clientBatchId,
  messages: request.parts.map((part, index): ConversationMessage => ({
    id: 999 + index,
    conversationId: request.conversationId,
    channelAccountId: 1,
    channel: 'zalo',
    accountDisplayName: 'Zalo Main',
    senderIdentityId: 1,
    senderDisplayName: 'Agent',
    direction: 'outbound',
    contentType: part.type,
    textContent: part.type === 'text' ? part.text : null,
    content: {},
    occurredAt: `2026-08-02T13:00:0${index}.000Z`,
    status: 'queued',
    removedAt: null,
    errorMessage: null,
    replyToMessageId: request.replyToMessageId,
    attachments: [],
    replyTarget: null,
    reactions: [],
    batchIndex: index,
    sendBatch: {
      sendBatchId: 'batch-optimistic',
      status: 'queued',
      totalParts: request.parts.length,
      sentParts: 0,
      failedParts: 0
    }
  }))
}));
const mockRecallConversationMessage = mock(async (messageId: number): Promise<ConversationMessage> => ({
  ...mockMsg1,
  id: messageId,
  direction: 'outbound',
  status: 'removed',
  removedAt: new Date().toISOString(),
  textContent: null
}));

const mockSetConversationReaction = mock(async (req: SetConversationReactionRequest) => ({
  clientMutationId: req.clientMutationId,
  messageId: req.messageId,
  reaction: req.reaction,
  requestedActive: req.active,
  status: 'succeeded' as const,
  reactions: [
    {
      reactionIcon: '❤️',
      count: 1,
      includesMe: true,
      actors: [{ identityId: 1, externalId: 'self', displayName: 'Self', avatarUrl: null }]
    }
  ],
  errorCode: null,
  errorMessage: null
}));

const mockForwardConversationMessage = mock(async (_request: any) => ({
  clientOperationId: 'op-1',
  sourceMessageId: 101,
  status: 'sent',
  targets: []
}));
const mockRetryConversationForwardTarget = mock(async (_request: any) => ({
  targetConversationId: 10,
  clientBatchId: 'b-1',
  status: 'sent',
  errorCode: null,
  errorMessage: null
}));

mock.module('$lib/services/conversationService', () => ({
  listConversationInbox: () => mockListConversationInbox(),
  getContactTimeline: (leadId: number, channels: any[], before: any, limit: number) =>
    mockGetContactTimeline(leadId, channels, before, limit),
  getConversationTimeline: (convId: number, before: any, limit: number) =>
    mockGetConversationTimeline(convId, before, limit),
  getIdentityTimeline: (identityId: number, before: any, limit: number) =>
    mockGetIdentityTimeline(identityId, before, limit),
  getConversationRuntimeStatus: () => mockGetConversationRuntimeStatus(),
  initConversationEventBridge: () => () => {},
  onConversationInvalidated: (listener: (event: ConversationInvalidatedEvent) => void) => {
    invalidationListeners.add(listener);
    return () => invalidationListeners.delete(listener);
  },
  sendConversationBatch: (request: SendConversationBatchRequest) => mockSendConversationBatch(request),
  stageConversationAttachmentPaths: mock(async () => []),
  stageConversationClipboardImage: mock(async () => null),
  discardConversationStagedAttachment: mock(async () => {}),
  restartConversationAccount: mock(async () => {}),
  searchZaloStickers: mock(async () => []),
  recallConversationMessage: (messageId: number) => mockRecallConversationMessage(messageId),
  setConversationReaction: (request: SetConversationReactionRequest) => mockSetConversationReaction(request),
  forwardConversationMessage: (request: any) => mockForwardConversationMessage(request),
  retryConversationForwardTarget: (request: any) => mockRetryConversationForwardTarget(request),
  linkConversationIdentity: mock(async () => {})
}));

mock.module('$lib/services/crmService', () => ({
  getLeads: mock(async () => []),
  getCategories: mock(async () => []),
  getCategoriesForLead: mock(async () => []),
  addLeadsToCategory: mock(async () => {}),
  removeLeadsFromCategory: mock(async () => {}),
  createCategory: mock(async () => 1),
  onLeadsChanged: mock((_listener: any) => () => {})
}));

describe('ConversationWorkspace Component', () => {
  afterEach(() => {
    cleanup();
    document.body.innerHTML = '';
    invalidationListeners.clear();
    mockListConversationInbox.mockClear();
    mockGetContactTimeline.mockClear();
    mockGetConversationTimeline.mockClear();
    mockGetIdentityTimeline.mockClear();
    mockGetConversationRuntimeStatus.mockClear();
    mockSendConversationBatch.mockClear();
    mockRecallConversationMessage.mockClear();
    mockSetConversationReaction.mockClear();
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

  test('3b. Selecting an unknown direct conversation activates its channel badge', async () => {
    mockListConversationInbox.mockImplementationOnce(async () => [mockUnknownItem]);

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((r) => setTimeout(r, 20));

    const zaloBadge = screen.getByRole('button', { name: 'zalo' });
    expect(zaloBadge.getAttribute('aria-pressed')).toBe('true');
    expect(screen.queryByText('Không có kênh nào đang được hiển thị')).toBeNull();
    expect(mockGetIdentityTimeline).toHaveBeenCalledWith(28, null, 50);
    expect(screen.queryByText('Chào shop')).not.toBeNull();
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

  test('6. The final active badge cannot be disabled', async () => {
    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((r) => setTimeout(r, 20));

    const zaloBadge = screen.getByRole('button', { name: 'zalo' });
    const messengerBadge = screen.getByRole('button', { name: 'messenger' });

    await fireEvent.click(zaloBadge);
    await fireEvent.click(messengerBadge);
    await tick();

    await new Promise((r) => setTimeout(r, 20));

    expect(screen.queryByText('Không có kênh nào đang được hiển thị')).toBeNull();
    expect(messengerBadge.getAttribute('aria-pressed')).toBe('true');
    expect(mockGetContactTimeline).toHaveBeenLastCalledWith(1, ['messenger'], null, 50);
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

  test('7b. Unknown pagination uses identity timeline cursor', async () => {
    const cursor = { occurredAt: '2026-07-31T10:00:00Z', messageId: 101 };
    mockListConversationInbox.mockImplementationOnce(async () => [mockUnknownItem]);
    mockGetIdentityTimeline.mockImplementationOnce(async () => ({
      items: [mockMsg1],
      nextCursor: cursor
    }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((r) => setTimeout(r, 20));

    mockGetIdentityTimeline.mockClear();

    const timelineContainer = document.querySelector('[data-timeline-container]');
    expect(timelineContainer).toBeTruthy();

    await fireEvent.scroll(timelineContainer!, { target: { scrollTop: 0 } });
    await tick();
    await new Promise((r) => setTimeout(r, 20));

    expect(mockGetIdentityTimeline).toHaveBeenCalledWith(28, cursor, 50);
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

  test('8b. Invalidation for an unknown reply target refreshes its identity timeline', async () => {
    mockListConversationInbox
      .mockImplementationOnce(async () => [mockUnknownItem])
      .mockImplementationOnce(async () => [mockUnknownItem]);

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((r) => setTimeout(r, 20));

    mockGetIdentityTimeline.mockClear();

    emitInvalidated({
      conversationId: 28,
      leadId: null,
      channel: 'zalo',
      reason: 'message',
      messageId: 103
    });

    await new Promise((r) => setTimeout(r, 70));

    expect(mockGetIdentityTimeline).toHaveBeenCalledWith(28, null, 50);
  });

  test('8c. Refresh replaces a stale failed bubble after the backend merges conversations', async () => {
    const staleFailedMessage: ConversationMessage = {
      ...mockMsg1,
      id: 428,
      direction: 'outbound',
      textContent: 'hohoho',
      content: { text: 'hohoho' },
      occurredAt: '2026-08-02T00:45:54Z',
      status: 'failed'
    };
    const restoredOldMessage: ConversationMessage = {
      ...mockMsg1,
      id: 426,
      direction: 'outbound',
      textContent: 'ua alo',
      content: { text: 'ua alo' },
      occurredAt: '2026-08-02T00:41:59Z',
      status: 'read'
    };
    const latestMessage: ConversationMessage = {
      ...mockMsg1,
      id: 429,
      direction: 'outbound',
      textContent: 'kekke',
      content: { text: 'kekke' },
      occurredAt: '2026-08-02T01:01:26Z',
      status: 'sent'
    };

    mockGetContactTimeline
      .mockImplementationOnce(async () => ({ items: [staleFailedMessage], nextCursor: null }))
      .mockImplementationOnce(async () => ({
        items: [restoredOldMessage, latestMessage],
        nextCursor: null
      }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(screen.getByText('hohoho')).toBeTruthy();

    emitInvalidated({
      conversationId: 10,
      leadId: 1,
      channel: 'zalo',
      reason: 'message',
      messageId: 429
    });

    await new Promise((resolve) => setTimeout(resolve, 80));

    expect(screen.queryByText('hohoho')).toBeNull();
    expect(screen.getByText('ua alo')).toBeTruthy();
    expect(screen.getByText('kekke')).toBeTruthy();
  });

  test('8d. Invalidation marks a cached closed conversation stale and refreshes it when selected', async () => {
    const refreshedGroupMessage: ConversationMessage = {
      ...mockMsg1,
      id: 855,
      conversationId: 30,
      direction: 'outbound',
      textContent: 'Nội dung đã chuyển tiếp',
      content: { text: 'Nội dung đã chuyển tiếp' },
      occurredAt: '2026-08-03T09:00:00Z',
      status: 'sent'
    };
    mockGetConversationTimeline
      .mockImplementationOnce(async () => ({
        items: [
          {
            ...mockMsg1,
            id: 201,
            conversationId: 30,
            textContent: 'Tin nhắn nhóm cũ',
            content: { text: 'Tin nhắn nhóm cũ' }
          }
        ],
        nextCursor: null
      }))
      .mockImplementationOnce(async () => ({ items: [refreshedGroupMessage], nextCursor: null }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 20));

    await fireEvent.click(screen.getByText('Nhóm VIP'));
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(screen.getByText('Tin nhắn nhóm cũ')).toBeTruthy();

    await fireEvent.click(screen.getAllByText('Nguyễn Văn A')[0]);
    await tick();
    mockGetConversationTimeline.mockClear();

    emitInvalidated({
      conversationId: 30,
      leadId: null,
      channel: 'zalo',
      reason: 'message',
      messageId: 855
    });
    await new Promise((resolve) => setTimeout(resolve, 70));
    expect(mockGetConversationTimeline).not.toHaveBeenCalled();

    await fireEvent.click(screen.getByText('Nhóm VIP'));
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 30));

    expect(mockGetConversationTimeline).toHaveBeenCalledWith(30, null, 50);
    expect(screen.getByText('Nội dung đã chuyển tiếp')).toBeTruthy();
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

  test('13. Reply action pins the composer to the source conversation/account', async () => {
    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 30));

    await fireEvent.click(screen.getAllByTestId('message-actions-trigger')[0]);
    await fireEvent.click(screen.getByText('Trả lời'));

    expect(screen.getByTestId('composer-reply-bar')).toBeTruthy();
    expect(screen.getByTestId('locked-target')).toBeTruthy();
    expect(screen.queryByTestId('channel-account-select')).toBeNull();
  });

  test('14. Recall confirmation replaces the cached message with the returned tombstone', async () => {
    mockGetContactTimeline.mockImplementationOnce(async () => ({
      items: [
        {
          ...mockMsg1,
          direction: 'outbound',
          status: 'sent',
          occurredAt: new Date().toISOString(),
          actions: { ...mockMsg1.actions!, recall: true }
        }
      ],
      nextCursor: null
    }));
    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 30));

    await fireEvent.click(screen.getByTestId('message-actions-trigger'));
    await fireEvent.click(screen.getByText('Thu hồi'));
    await fireEvent.click(screen.getByRole('button', { name: 'Xác nhận thu hồi' }));
    await tick();

    expect(mockRecallConversationMessage).toHaveBeenCalledTimes(1);
    expect(screen.getByText('Tin nhắn đã bị xóa')).toBeTruthy();
  });

  test('15. Toggling reaction calls setConversationReaction and updates reactions on success', async () => {
    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 30));

    await fireEvent.click(screen.getAllByTestId('message-actions-trigger')[0]);
    await fireEvent.click(screen.getByRole('button', { name: /Thả cảm xúc ❤️/i }));
    await tick();

    expect(mockSetConversationReaction).toHaveBeenCalledTimes(1);
    expect(mockSetConversationReaction.mock.calls[0][0].reaction).toBe('heart');
    expect(mockSetConversationReaction.mock.calls[0][0].active).toBe(true);
  });

  test('16. Clicking "Chuyển tiếp" opens the ForwardConversationDialog with message preview', async () => {
    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 30));

    await fireEvent.click(screen.getAllByTestId('message-actions-trigger')[0]);
    const forwardOption = screen.getByText('Chuyển tiếp');
    expect(forwardOption).toBeTruthy();

    await fireEvent.click(forwardOption);
    await tick();

    expect(await screen.findByTestId('forward-conversation-dialog')).toBeTruthy();
    expect(screen.getByText('Chuyển tiếp tin nhắn')).toBeTruthy();
    expect(screen.getAllByText('Chào shop').length).toBeGreaterThanOrEqual(1);
  });

  test('17. Clicking an image message opens the ImageGalleryModal with image preview', async () => {
    mockGetContactTimeline.mockImplementation(() => Promise.resolve({
      items: [
        {
          id: 101,
          conversationId: 10,
          channelAccountId: 1,
          channel: 'zalo',
          accountDisplayName: 'Zalo Main',
          senderIdentityId: 100,
          senderDisplayName: 'Nguyễn Văn A',
          direction: 'inbound',
          contentType: 'image',
          textContent: 'Ảnh sản phẩm',
          content: { url: 'https://cdn.example.com/photo1.png' },
          occurredAt: '2026-07-31T10:00:00Z',
          status: 'received',
          removedAt: null,
          errorMessage: null,
          replyToMessageId: null,
          attachments: [
            {
              id: 1,
              mediaType: 'image',
              fileName: 'photo1.png',
              remoteUrl: 'https://cdn.example.com/photo1.png',
              fileSizeBytes: 1000,
              sizeBytes: 1000,
              thumbnailUrl: null,
              width: null,
              height: null,
              mimeType: 'image/png',
              durationSeconds: null
            }
          ],
          replyTarget: null,
          reactions: [],
          batchIndex: null,
          sendBatch: null,
          actions: { reply: true, recall: false, forward: true, react: true, copy: true }
        }
      ],
      nextCursor: null
    }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 30));

    const imageContent = screen.getByTestId('image-message-content');
    expect(imageContent).toBeTruthy();

    await fireEvent.click(imageContent);
    await tick();

    expect(screen.getByTestId('image-gallery-modal')).toBeTruthy();
    expect(screen.getByTestId('gallery-main-image').getAttribute('src')).toBe('https://cdn.example.com/photo1.png');
  });
});
