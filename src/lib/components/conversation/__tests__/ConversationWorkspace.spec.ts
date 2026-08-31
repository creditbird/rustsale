import { afterEach, beforeAll, beforeEach, describe, expect, test, mock } from 'bun:test';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { tick } from 'svelte';
import { setLocale } from '$lib/i18n';
import type {
  InboxItem,
  ConversationRuntimeStatus,
  ConversationMessage,
  TimelinePage,
  ConversationInvalidatedEvent,
  ConversationMessageDeltaEvent,
  ConversationCapabilities,
  SendConversationBatchRequest,
  SetConversationReactionRequest
} from '$lib/services/conversationService';
import ConversationWorkspace from '../ConversationWorkspace.svelte';
import * as m from '$lib/paraglide/messages';

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
let deltaListeners = new Set<(event: ConversationMessageDeltaEvent) => void>();
let lagListeners = new Set<(payload: { skipped: number }) => void>();
let reconnectListeners = new Set<() => void>();

function emitLagged(skipped: number) {
  for (const l of lagListeners) l({ skipped });
}
function emitReconnect() {
  for (const l of reconnectListeners) l();
}

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

function emitDelta(event: ConversationMessageDeltaEvent) {
  for (const listener of deltaListeners) {
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
    { identityId: 1, externalId: 'member-1', displayName: 'Minh', avatarUrl: null, participantRole: 'member' }
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
const mockGetGroupMembersUnrestricted = mock(async (_conversationId: number) => [
  {
    identity_id: 1,
    external_id: 'uid-a',
    display_name: 'Member A',
    avatar_url: null,
    participant_role: 'member',
    lead_id: null,
    lead_name: null,
    categories: []
  }
]);
const mockSyncAndDeepScanGroupMembers = mock(async (_conversationId: number) => ({
  members: [],
  discovered_count: 0,
  profile_hydrated_count: 0,
  membership_complete: false,
  pages_attempted: 0,
  pages_succeeded: 0,
  messages_scanned: 0,
  stop_reason: null,
  warning: null,
  diagnostics: {
    direct_snapshot: { attempted: false, complete: false, member_count: 0, warning: null },
    board: {
      attempted: false,
      pages_attempted: 0,
      pages_succeeded: 0,
      items_inspected: 0,
      polls_attempted: 0,
      polls_succeeded: 0,
      committed_uid_count: 0,
      stop_reason: null,
      warnings: []
    },
    history_backfill: {
      provider_account_key: 'zalo-1',
      scope: 'account_group_messages',
      outcome: 'unavailable',
      joined_existing_run: false,
      pages_requested: 0,
      pages_received: 0,
      start_cursor: null,
      durable_cursor: null,
      stop_reason: null,
      warning: null
    },
    local_evidence: {
      messages_scanned: 0,
      participant_uid_count: 0,
      sender_uid_count: 0,
      mention_uid_count: 0,
      reaction_uid_count: 0,
      quote_owner_uid_count: 0,
      total_uid_count: 0,
      committed_uid_count: 0,
      rejected_uid_count: 0
    },
    profile_hydration: { attempted: 0, succeeded: 0, failed: 0, warnings: [] },
    partial_result: true,
    warnings: []
  }
}));
const mockBatchImportGroupMembersToLeads = mock(async (_request: unknown) => ({
  imported_count: 0,
  updated_count: 0,
  skipped_count: 0,
  assigned_category_ids: []
}));
const mockListConversationMentionCandidates = mock(
  async (_request: unknown): Promise<import('$lib/services/conversationService').ConversationMentionCandidate[]> => []
);
const mockStartDirectConversation = mock(
  async (_request: any): Promise<any> => ({
    id: 501,
    conversationId: 888,
    channelAccountId: 1,
    channel: 'zalo' as const,
    accountDisplayName: 'Zalo Main',
    senderIdentityId: 100,
    senderDisplayName: 'Admin',
    direction: 'outbound' as const,
    contentType: 'text',
    textContent: 'Hello',
    content: { text: 'Hello' },
    occurredAt: '2026-08-14T10:05:00Z',
    status: 'sent' as const,
    removedAt: null,
    errorMessage: null,
    replyToMessageId: null,
    attachments: [],
    replyTarget: null,
    reactions: [],
    batchIndex: null,
    sendBatch: null,
    mentions: [],
    actions: { reply: true, recall: false, forward: true, react: true, copy: true }
  })
);

const mockResumeConversationDeltas = mock(
  async (_afterSeq: number, _limit?: number): Promise<ConversationMessageDeltaEvent[]> => []
);
const mockGetConversationHighWaterMark = mock(
  async () => ({ currentSequence: 0, minSequence: 0 })
);

mock.module('$lib/services/conversationService', () => ({
  listConversationInbox: () => mockListConversationInbox(),
  getContactTimeline: (leadId: number, channels: any[], before: any, limit: number) =>
    mockGetContactTimeline(leadId, channels, before, limit),
  getConversationTimeline: (convId: number, before: any, limit: number) =>
    mockGetConversationTimeline(convId, before, limit),
  getIdentityTimeline: (identityId: number, before: any, limit: number) =>
    mockGetIdentityTimeline(identityId, before, limit),
  getConversationRuntimeStatus: () => mockGetConversationRuntimeStatus(),
  getConversationHighWaterMark: () => mockGetConversationHighWaterMark(),
  initConversationEventBridge: () => () => {},
  onConversationInvalidated: (listener: (event: ConversationInvalidatedEvent) => void) => {
    invalidationListeners.add(listener);
    return () => invalidationListeners.delete(listener);
  },
  onConversationMessageDelta: (listener: (event: ConversationMessageDeltaEvent) => void) => {
    deltaListeners.add(listener);
    return () => deltaListeners.delete(listener);
  },
  onConversationDeltaLagged: (listener: (payload: { skipped: number }) => void) => {
    lagListeners.add(listener);
    return () => lagListeners.delete(listener);
  },
  onConversationBridgeReconnected: (listener: () => void) => {
    reconnectListeners.add(listener);
    return () => reconnectListeners.delete(listener);
  },
  resumeConversationDeltas: (afterSeq: number, limit?: number) =>
    mockResumeConversationDeltas(afterSeq, limit),
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
  linkConversationIdentity: mock(async () => {}),
  listConversationMentionCandidates: (request: unknown) =>
    mockListConversationMentionCandidates(request),
  startDirectConversation: (request: any) => mockStartDirectConversation(request),
  getGroupMembersUnrestricted: (conversationId: number) =>
    mockGetGroupMembersUnrestricted(conversationId),
  syncAndDeepScanGroupMembers: (conversationId: number) =>
    mockSyncAndDeepScanGroupMembers(conversationId),
  batchImportGroupMembersToLeads: (request: unknown) =>
    mockBatchImportGroupMembersToLeads(request),
  syncZaloGroupChatHistory: mock(async (_conversationId: number) => 0),
  pinConversationMessage: mock(async () => {}),
  unpinConversationMessage: mock(async () => {}),
  listConversationPinnedMessages: mock(async () => []),
  syncFacebookConversations: mock(async () => 0),
  syncWhatsAppConversations: mock(async () => 0)
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

mock.module('$lib/services/metaRealtimeService', () => ({
  startMetaRealtimeListener: mock(() => () => {}),
  stopMetaRealtimeListener: mock(() => {}),
  schedulePageSync: mock(() => {})
}));

describe('ConversationWorkspace Component', () => {
  beforeEach(() => {
    setLocale('en');
    mockGetConversationHighWaterMark.mockImplementation(async () => ({
      currentSequence: 0,
      minSequence: 0
    }));
    mockResumeConversationDeltas.mockImplementation(async () => []);
  });

  afterEach(() => {
    cleanup();
    document.body.innerHTML = '';
    invalidationListeners.clear();
    deltaListeners.clear();
    lagListeners.clear();
    reconnectListeners.clear();
    mockListConversationInbox.mockClear();
    mockGetContactTimeline.mockClear();
    mockGetConversationTimeline.mockClear();
    mockGetIdentityTimeline.mockClear();
    mockGetConversationRuntimeStatus.mockClear();
    mockGetConversationHighWaterMark.mockClear();
    mockResumeConversationDeltas.mockClear();
    mockSendConversationBatch.mockClear();
    mockRecallConversationMessage.mockClear();
    mockSetConversationReaction.mockClear();
    mockGetGroupMembersUnrestricted.mockClear();
    mockSyncAndDeepScanGroupMembers.mockClear();
    mockBatchImportGroupMembersToLeads.mockClear();
    mockListConversationMentionCandidates.mockClear();
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
    expect(screen.queryByText('No channels currently visible')).toBeNull();
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
    expect(screen.queryByText('No channels currently visible')).toBeNull();
  });

  test('4b. Group member dialog traps focus, closes with Escape, and restores the trigger', async () => {
    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 20));

    await fireEvent.click(screen.getByText('Nhóm VIP'));
    await tick();
    const trigger = screen.getByRole('button', { name: /3 members/ });
    trigger.focus();
    await fireEvent.click(trigger);

    const dialog = await screen.findByRole('dialog', { name: 'Nhóm VIP' });
    await waitFor(() => expect(dialog.contains(document.activeElement)).toBe(true));

    const dialogButtons = within(dialog).getAllByRole('button');
    const lastButton = dialogButtons.at(-1) as HTMLButtonElement;
    lastButton.focus();
    await fireEvent.keyDown(lastButton, { key: 'Tab' });
    expect(dialog.contains(document.activeElement)).toBe(true);

    await fireEvent.keyDown(document, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Nhóm VIP' })).toBeNull());
    expect(document.activeElement).toBe(trigger);
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

    expect(screen.queryByText('No channels currently visible')).toBeNull();
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
      reason: 'history_sync',
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
      reason: 'history_sync',
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
      reason: 'history_sync',
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
      reason: 'history_sync',
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

  test('8e. Invalidation updates open timeline in real-time even when composer is focused', async () => {
    const realtimeMsg: ConversationMessage = {
      ...mockMsg1,
      id: 999,
      conversationId: 10,
      direction: 'inbound',
      textContent: 'Tin nhắn realtime tức thì',
      content: { text: 'Tin nhắn realtime tức thì' },
      occurredAt: '2026-08-28T10:00:00Z',
      status: 'received'
    };
    mockGetContactTimeline
      .mockImplementationOnce(async () => ({ items: [mockMsg1], nextCursor: null }))
      .mockImplementationOnce(async () => ({ items: [mockMsg1, realtimeMsg], nextCursor: null }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(screen.getByText('Xin chào')).toBeTruthy();

    const textarea = screen.getByPlaceholderText('Type a message...');
    textarea.focus();
    expect(document.activeElement).toBe(textarea);

    emitInvalidated({
      conversationId: 10,
      leadId: 1,
      channel: 'zalo',
      reason: 'history_sync',
      messageId: 999
    });

    await new Promise((resolve) => setTimeout(resolve, 40));

    expect(screen.getByText('Tin nhắn realtime tức thì')).toBeTruthy();
  });

  test('8f. Invalidation for open customer with leadId = null still refreshes timeline via conversationId match', async () => {
    const unlinkedZaloMsg: ConversationMessage = {
      ...mockMsg1,
      id: 1001,
      conversationId: 10,
      channel: 'zalo',
      direction: 'inbound',
      textContent: 'Tin nhắn Zalo leadId null',
      content: { text: 'Tin nhắn Zalo leadId null' },
      occurredAt: '2026-08-28T10:05:00Z',
      status: 'received'
    };
    mockGetContactTimeline
      .mockImplementationOnce(async () => ({ items: [mockMsg1], nextCursor: null }))
      .mockImplementationOnce(async () => ({ items: [mockMsg1, unlinkedZaloMsg], nextCursor: null }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(screen.getByText('Xin chào')).toBeTruthy();

    emitInvalidated({
      conversationId: 10,
      leadId: null,
      channel: 'zalo',
      reason: 'history_sync',
      messageId: 1001
    });

    await new Promise((resolve) => setTimeout(resolve, 60));

    expect(screen.getByText('Tin nhắn Zalo leadId null')).toBeTruthy();
  });

  test('8g. Non-discarding merge renders new message even if trailing receipt events arrive during IPC fetch', async () => {
    const freshMsg: ConversationMessage = {
      ...mockMsg1,
      id: 1002,
      conversationId: 10,
      direction: 'inbound',
      textContent: 'Tin nhắn ngay trước receipt',
      content: { text: 'Tin nhắn ngay trước receipt' },
      occurredAt: '2026-08-28T10:10:00Z',
      status: 'received'
    };
    mockGetContactTimeline
      .mockImplementationOnce(async () => ({ items: [mockMsg1], nextCursor: null }))
      .mockImplementationOnce(async () => {
        // Simulate trailing receipt event arriving during in-flight IPC fetch
        emitInvalidated({
          conversationId: 10,
          leadId: 1,
          channel: 'zalo',
          reason: 'receipt',
          messageId: 1002
        });
        return { items: [mockMsg1, freshMsg], nextCursor: null };
      })
      .mockImplementation(async () => ({ items: [mockMsg1, freshMsg], nextCursor: null }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(screen.getByText('Xin chào')).toBeTruthy();

    emitInvalidated({
      conversationId: 10,
      leadId: 1,
      channel: 'zalo',
      reason: 'history_sync',
      messageId: 1002
    });

    await new Promise((resolve) => setTimeout(resolve, 80));

    expect(screen.getByText('Tin nhắn ngay trước receipt')).toBeTruthy();
  });

  test('8h. Invalidation with mismatched event.leadId but matching item.replyTargets still refreshes open timeline instantly', async () => {
    const mismatchLeadMsg: ConversationMessage = {
      ...mockMsg1,
      id: 1003,
      conversationId: 10,
      channel: 'zalo',
      direction: 'inbound',
      textContent: 'Tin nhắn mismatch lead id',
      content: { text: 'Tin nhắn mismatch lead id' },
      occurredAt: '2026-08-28T10:10:00Z',
      status: 'received'
    };
    mockGetContactTimeline
      .mockImplementationOnce(async () => ({ items: [mockMsg1], nextCursor: null }))
      .mockImplementationOnce(async () => ({ items: [mockMsg1, mismatchLeadMsg], nextCursor: null }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(screen.getByText('Xin chào')).toBeTruthy();

    emitInvalidated({
      conversationId: 10,
      leadId: 999, // Mismatched leadId from backend participant ambiguity
      channel: 'zalo',
      reason: 'history_sync',
      messageId: 1003
    });

    await new Promise((resolve) => setTimeout(resolve, 60));

    expect(screen.getByText('Tin nhắn mismatch lead id')).toBeTruthy();
  });

  test('8i. Direct ConversationMessageDeltaEvent renders new bubble immediately without requiring IPC timeline refetch', async () => {
    const deltaMsg: ConversationMessage = {
      ...mockMsg1,
      id: 2005,
      conversationId: 10,
      channel: 'zalo',
      direction: 'inbound',
      textContent: 'Tin nhắn Delta tức thì 0ms',
      content: { text: 'Tin nhắn Delta tức thì 0ms' },
      occurredAt: '2026-08-28T10:15:00Z',
      status: 'received'
    };

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(screen.getByText('Xin chào')).toBeTruthy();

    mockGetContactTimeline.mockClear();

    // Emit delta directly
    emitDelta({
      sequence: 1,
      conversationId: 10,
      leadId: 1,
      channel: 'zalo',
      message: deltaMsg,
      emittedAtMs: Date.now()
    });

    await tick();
    await new Promise((resolve) => setTimeout(resolve, 20));

    // Verify DOM contains the new bubble in open timeline AND updated sidebar preview
    const matchingElements = screen.getAllByText('Tin nhắn Delta tức thì 0ms');
    expect(matchingElements.length).toBe(2);
    // Verify getContactTimeline IPC was NOT called (0 IPC round-trips for immediate render)
    expect(mockGetContactTimeline).not.toHaveBeenCalled();
  });

  test('8j. Outbound send displays optimistic message bubble immediately and reconciles seamlessly with server batch', async () => {
    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(screen.getByText('Xin chào')).toBeTruthy();

    const textarea = screen.getByPlaceholderText(/Type a message|Send message/i) as HTMLTextAreaElement;
    await fireEvent.input(textarea, { target: { value: 'Tin nhắn gửi đi tức thì 0ms' } });
    await fireEvent.keyDown(textarea, { key: 'Enter', shiftKey: false });
    await tick();

    // Verify optimistic message is immediately in DOM in both timeline and sidebar preview
    const matching = screen.getAllByText('Tin nhắn gửi đi tức thì 0ms');
    expect(matching.length).toBeGreaterThanOrEqual(2);
    expect(textarea.value).toBe('');
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

    const textarea = screen.getByPlaceholderText(/Type a message|Send message/i);
    await fireEvent.input(textarea, { target: { value: 'Tin nhắn gửi trực tiếp' } });

    const sendBtn = screen.getByRole('button', { name: /Send/i });
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

    expect(screen.getByRole('button', { name: /Change Lead/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /Unlink/i })).toBeTruthy();
  });

  test('13. Reply action pins the composer to the source conversation/account', async () => {
    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 30));

    await fireEvent.click(screen.getAllByTestId('message-actions-trigger')[0]);
    await fireEvent.click(screen.getByText('Reply'));

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
    await fireEvent.click(screen.getByText(m.conversation_recall()));
    await fireEvent.click(screen.getByRole('button', { name: m.conversation_recall_confirm() }));
    await tick();

    expect(mockRecallConversationMessage).toHaveBeenCalledTimes(1);
    expect(screen.getByText(m.conversation_message_recalled())).toBeTruthy();
  });

  test('15. Toggling reaction calls setConversationReaction and updates reactions on success', async () => {
    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 30));

    await fireEvent.click(screen.getAllByTestId('message-actions-trigger')[0]);
    await fireEvent.click(screen.getByRole('button', { name: /React ❤️/i }));
    await tick();

    expect(mockSetConversationReaction).toHaveBeenCalledTimes(1);
    expect(mockSetConversationReaction.mock.calls[0][0].reaction).toBe('heart');
    expect(mockSetConversationReaction.mock.calls[0][0].active).toBe(true);
  });

  test('15b. Clicking an existing self reaction sends another provider add', async () => {
    mockGetContactTimeline.mockImplementationOnce(async () => ({
      items: [
        {
          ...mockMsg1,
          reactions: [
            {
              reactionIcon: '❤️',
              count: 1,
              includesMe: true,
              actors: [
                { identityId: 1, externalId: 'self', displayName: 'Self', avatarUrl: null }
              ]
            }
          ]
        }
      ],
      nextCursor: null
    }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 30));

    await fireEvent.click(screen.getByRole('button', { name: /❤️ 1/ }));
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
    const forwardOption = screen.getByText('Forward');
    expect(forwardOption).toBeTruthy();

    await fireEvent.click(forwardOption);
    await tick();

    expect(await screen.findByTestId('forward-conversation-dialog')).toBeTruthy();
    expect(screen.getByText('Forward Message')).toBeTruthy();
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

  test('18. Clicking a tagged user mention in a group message resolves candidate details and opens UserProfilePopover', async () => {
    mockListConversationMentionCandidates.mockImplementationOnce(async () => [
      {
        identityId: 3230066,
        externalId: '5054685013624794058',
        displayName: 'Nga Nguyễn B B Tourist',
        avatarUrl: null,
        participantRole: 'member'
      }
    ]);
    mockGetConversationTimeline.mockImplementationOnce(async () => ({
      items: [
        {
          id: 99,
          conversationId: 30,
          channelAccountId: 1,
          channel: 'zalo',
          accountDisplayName: 'Zalo Main',
          senderIdentityId: 100,
          senderDisplayName: 'Nguyễn Văn A',
          direction: 'inbound',
          contentType: 'text',
          textContent: '@Nga Nguyễn B B Tourist',
          content: { text: '@Nga Nguyễn B B Tourist' },
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
          mentions: [],
          actions: { reply: true, recall: false, forward: true, react: true, copy: true }
        }
      ],
      nextCursor: null
    }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 30));

    // Select group conversation 30 ('Nhóm VIP')
    const groupItem = screen.getByText('Nhóm VIP');
    await fireEvent.click(groupItem);
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 30));

    const mentionEl = screen.getByText('@Nga Nguyễn B');
    expect(mentionEl).toBeTruthy();

    await fireEvent.click(mentionEl);
    await tick();

    expect(mockListConversationMentionCandidates).toHaveBeenCalledWith({
      conversationId: 30,
      sourceMessageId: 99,
      query: 'Nga Nguyễn B'
    });
    expect(screen.getByText('UID: 5054685013624794058')).toBeTruthy();
    await fireEvent.click(screen.getByTestId('btn-chat-1on1'));

    const directRecipientInput = await screen.findByPlaceholderText('e.g. 0987654321 or Zalo UID');
    expect((directRecipientInput as HTMLInputElement).value).toBe('5054685013624794058');
  });

  test('18b. Internal identity IDs stored as mention external IDs are replaced by the group candidate Zalo UID', async () => {
    mockListConversationMentionCandidates.mockImplementationOnce(async () => [
      {
        identityId: 3230066,
        externalId: '5054685013624794058',
        displayName: 'Nga Nguyễn B B Tourist',
        avatarUrl: null,
        participantRole: 'member'
      }
    ]);
    mockGetConversationTimeline.mockImplementationOnce(async () => ({
      items: [
        {
          id: 100,
          conversationId: 30,
          channelAccountId: 1,
          channel: 'zalo',
          accountDisplayName: 'Zalo Main',
          senderIdentityId: 100,
          senderDisplayName: 'Nguyễn Văn A',
          direction: 'inbound',
          contentType: 'text',
          textContent: '@Nga Nguyễn B B Tourist',
          content: { text: '@Nga Nguyễn B B Tourist' },
          occurredAt: '2026-08-13T08:57:01Z',
          status: 'received',
          removedAt: null,
          errorMessage: null,
          replyToMessageId: null,
          attachments: [],
          replyTarget: null,
          reactions: [],
          batchIndex: null,
          sendBatch: null,
          mentions: [
            {
              identityId: 3230066,
              externalId: '3230066',
              displayText: '@Nga Nguyễn B',
              startUtf16: 0,
              lengthUtf16: 13
            }
          ],
          actions: { reply: true, recall: false, forward: true, react: true, copy: true }
        }
      ],
      nextCursor: null
    }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 30));

    await fireEvent.click(screen.getByText('Nhóm VIP'));
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 30));
    await fireEvent.click(screen.getByText('@Nga Nguyễn B'));
    await tick();

    expect(mockListConversationMentionCandidates).toHaveBeenCalled();
    expect(screen.getByText('UID: 5054685013624794058')).toBeTruthy();
  });

  test('18c. Internal identity IDs are not shown as Zalo UIDs when no mention candidate is found', async () => {
    mockListConversationMentionCandidates.mockImplementationOnce(async () => []);
    mockGetConversationTimeline.mockImplementationOnce(async () => ({
      items: [
        {
          id: 101,
          conversationId: 30,
          channelAccountId: 1,
          channel: 'zalo',
          accountDisplayName: 'Zalo Main',
          senderIdentityId: 100,
          senderDisplayName: 'Nguyễn Văn A',
          direction: 'inbound',
          contentType: 'text',
          textContent: '@Nga Nguyễn B',
          content: { text: '@Nga Nguyễn B' },
          occurredAt: '2026-08-13T09:15:00Z',
          status: 'received',
          removedAt: null,
          errorMessage: null,
          replyToMessageId: null,
          attachments: [],
          replyTarget: null,
          reactions: [],
          batchIndex: null,
          sendBatch: null,
          mentions: [
            {
              identityId: 3230066,
              externalId: '3230066',
              displayText: '@Nga Nguyễn B',
              startUtf16: 0,
              lengthUtf16: 13
            }
          ],
          actions: { reply: true, recall: false, forward: true, react: true, copy: true }
        }
      ],
      nextCursor: null
    }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 30));

    await fireEvent.click(screen.getByText('Nhóm VIP'));
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 30));
    await fireEvent.click(screen.getByText('@Nga Nguyễn B'));
    await tick();

    expect(mockListConversationMentionCandidates).toHaveBeenCalled();
    expect(screen.queryByText('UID: 3230066')).toBeNull();
  });

  test('18d. Historical reply aliases resolve the quoted user by source message ID', async () => {
    mockListConversationMentionCandidates.mockImplementationOnce(async (request: any) =>
      request.sourceMessageId === 4006
        ? [
            {
              identityId: 142,
              externalId: '5025114793295720787',
              displayName: 'Nct',
              avatarUrl: null,
              participantRole: 'reply_sender'
            }
          ]
        : []
    );
    mockGetConversationTimeline.mockImplementationOnce(async () => ({
      items: [
        {
          id: 4006,
          conversationId: 30,
          channelAccountId: 1,
          channel: 'zalo',
          accountDisplayName: 'Hello World',
          senderIdentityId: 49,
          senderDisplayName: 'Din',
          direction: 'inbound',
          contentType: 'text',
          textContent: '@Nct còn không bác rep em với hic',
          content: { text: '@Nct còn không bác rep em với hic' },
          occurredAt: '2026-08-13T02:04:27.537Z',
          status: 'received',
          removedAt: null,
          errorMessage: null,
          replyToMessageId: null,
          attachments: [],
          replyTarget: null,
          reactions: [],
          batchIndex: null,
          sendBatch: null,
          mentions: [],
          actions: { reply: true, recall: false, forward: true, react: true, copy: true }
        }
      ],
      nextCursor: null
    }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 30));

    await fireEvent.click(screen.getByText('Nhóm VIP'));
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 30));
    await fireEvent.click(screen.getByText('@Nct'));
    await tick();

    expect(mockListConversationMentionCandidates).toHaveBeenCalledWith({
      conversationId: 30,
      query: 'Nct',
      sourceMessageId: 4006
    });
    expect(screen.getByText('UID: 5025114793295720787')).toBeTruthy();
    await fireEvent.click(screen.getByTestId('btn-chat-1on1'));

    const directRecipientInput = await screen.findByPlaceholderText('e.g. 0987654321 or Zalo UID');
    expect((directRecipientInput as HTMLInputElement).value).toBe('5025114793295720787');
  });

  test('18e. Clicking View / Create Lead in UserProfilePopover opens CreateLeadDialog with pre-filled candidate Zalo UID and name', async () => {
    mockListConversationMentionCandidates.mockImplementationOnce(async () => [
      {
        identityId: 3230066,
        externalId: '5054685013624794058',
        displayName: 'Little Cat',
        avatarUrl: null,
        participantRole: 'member'
      }
    ]);
    mockGetConversationTimeline.mockImplementationOnce(async () => ({
      items: [
        {
          id: 200,
          conversationId: 30,
          channelAccountId: 1,
          channel: 'zalo',
          accountDisplayName: 'Zalo Main',
          senderIdentityId: 100,
          senderDisplayName: 'Nguyễn Văn A',
          direction: 'inbound',
          contentType: 'text',
          textContent: '@Little Cat how',
          content: { text: '@Little Cat how' },
          occurredAt: '2026-08-14T10:00:00Z',
          status: 'received',
          removedAt: null,
          errorMessage: null,
          replyToMessageId: null,
          attachments: [],
          replyTarget: null,
          reactions: [],
          batchIndex: null,
          sendBatch: null,
          mentions: [],
          actions: { reply: true, recall: false, forward: true, react: true, copy: true }
        }
      ],
      nextCursor: null
    }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 30));

    await fireEvent.click(screen.getByText('Nhóm VIP'));
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 30));

    await fireEvent.click(screen.getByText('@Little Cat'));
    await tick();

    expect(screen.getByText('UID: 5054685013624794058')).toBeTruthy();
    const leadBtn = screen.getByTestId('btn-lead-crm');
    await fireEvent.click(leadBtn);
    await tick();

    expect(screen.getByText('Create New CRM Lead')).toBeTruthy();
    const nameInput = screen.getByDisplayValue('Little Cat') as HTMLInputElement;
    const phoneInput = screen.getByDisplayValue('5054685013624794058') as HTMLInputElement;
    expect(nameInput).toBeTruthy();
    expect(phoneInput).toBeTruthy();
  });

  test('18f. Creating a new direct conversation selects the new contact chat via replyTargets instead of selecting the first group conversation', async () => {
    const directCreatedItem = {
      key: 'identity:999',
      kind: 'unknown' as const,
      leadId: null,
      conversationId: null,
      identityId: 999,
      title: 'Ktv Dung',
      avatarUrl: null,
      lastMessagePreview: 'Hello Ktv Dung',
      lastMessageAt: '2026-08-14T10:05:00Z',
      channels: [{ channel: 'zalo' as const, messageCount: 1, unreadCount: 0 }],
      replyTargets: [
        {
          conversationId: 888,
          channelAccountId: 1,
          channel: 'zalo' as const,
          accountDisplayName: 'Zalo Main',
          conversationKind: 'direct' as const,
          lastInboundAt: null,
          lastMessageAt: '2026-08-14T10:05:00Z'
        }
      ],
      participantCount: 0,
      participants: []
    };

    mockListConversationInbox.mockImplementation(async () => [
      // First item in inbox is a group
      {
        key: 'conversation:30',
        kind: 'group' as const,
        leadId: null,
        conversationId: 30,
        identityId: null,
        title: 'Nhóm VIP',
        avatarUrl: null,
        lastMessagePreview: 'Group message',
        lastMessageAt: '2026-08-14T10:00:00Z',
        channels: [{ channel: 'zalo' as const, messageCount: 5 }],
        replyTargets: [],
        participantCount: 5,
        participants: []
      },
      directCreatedItem
    ]);

    mockStartDirectConversation.mockImplementationOnce(async () => ({
      id: 501,
      conversationId: 888,
      channelAccountId: 1,
      channel: 'zalo' as const,
      accountDisplayName: 'Zalo Main',
      senderIdentityId: 100,
      senderDisplayName: 'Admin',
      direction: 'outbound' as const,
      contentType: 'text',
      textContent: 'Hello Ktv Dung',
      content: { text: 'Hello Ktv Dung' },
      occurredAt: '2026-08-14T10:05:00Z',
      status: 'sent' as const,
      removedAt: null,
      errorMessage: null,
      replyToMessageId: null,
      attachments: [],
      replyTarget: null,
      reactions: [],
      batchIndex: null,
      sendBatch: null,
      mentions: [],
      actions: { reply: true, recall: false, forward: true, react: true, copy: true }
    }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 30));

    // Open new chat dialog from sidebar
    const newChatBtn = screen.getByText('New Chat');
    await fireEvent.click(newChatBtn);
    await tick();

    const phoneInput = screen.getByPlaceholderText('e.g. 0987654321 or Zalo UID') as HTMLInputElement;
    const msgInput = screen.getByPlaceholderText('Enter initial message to send to customer...') as HTMLTextAreaElement;

    await fireEvent.input(phoneInput, { target: { value: '0987654321' } });
    await fireEvent.input(msgInput, { target: { value: 'Hello Ktv Dung' } });
    await tick();

    const sendBtn = screen.getByText('Start Chat');
    await fireEvent.click(sendBtn);
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));

    // Header must show Ktv Dung, not Nhóm VIP
    const headerTitle = screen.getByRole('heading', { level: 2 });
    expect(headerTitle.textContent).toBe('Ktv Dung');
  });

  test('20. Omnichannel WhatsApp conversation displays WhatsApp badges and renders timeline messages', async () => {
    const whatsappLeadItem: InboxItem = {
      key: 'lead:99',
      kind: 'customer',
      leadId: 99,
      conversationId: null,
      identityId: null,
      title: 'WhatsApp Customer (+84988776655)',
      avatarUrl: null,
      lastMessagePreview: 'Inquiry via WhatsApp',
      lastMessageAt: '2026-08-02T11:00:00Z',
      channels: [
        { channel: 'whatsapp', messageCount: 5 },
        { channel: 'zalo', messageCount: 2 }
      ],
      replyTargets: [
        {
          conversationId: 990,
          channelAccountId: 50,
          channel: 'whatsapp',
          accountDisplayName: 'WhatsApp Sales',
          conversationKind: 'direct',
          lastInboundAt: '2026-08-02T11:00:00Z',
          lastMessageAt: '2026-08-02T11:00:00Z'
        }
      ],
      participantCount: 1,
      participants: []
    };

    mockListConversationInbox.mockImplementation(async () => [whatsappLeadItem]);
    mockGetContactTimeline.mockImplementation(async () => ({
      items: [
        {
          id: 9901,
          conversationId: 990,
          channelAccountId: 50,
          channel: 'whatsapp',
          accountDisplayName: 'WhatsApp Sales',
          senderIdentityId: 990,
          senderDisplayName: '+84988776655',
          direction: 'inbound',
          contentType: 'text',
          textContent: 'Hi, I need pricing information.',
          content: {},
          occurredAt: '2026-08-02T11:00:00Z',
          status: 'received',
          removedAt: null,
          errorMessage: null,
          replyToMessageId: null,
          attachments: [],
          replyTarget: null,
          reactions: [],
          batchIndex: null,
          sendBatch: null,
          mentions: [],
          actions: { reply: true, recall: false, forward: true, react: true, copy: true }
        }
      ],
      hasMore: false,
      nextCursor: null
    }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(screen.getAllByText('WhatsApp Customer (+84988776655)').length).toBeGreaterThan(0);
    expect(screen.getByText('Hi, I need pricing information.')).toBeTruthy();

    const waBadge = screen.getByRole('button', { name: 'whatsapp' });
    expect(waBadge).toBeTruthy();
    expect(waBadge.getAttribute('aria-pressed')).toBe('true');
  });

  test('21. Inbound delta upserts directly into timeline and suppresses redundant 50-message IPC reload on message invalidation', async () => {
    mockListConversationInbox.mockImplementation(async () => [mockCustomerItem]);
    let timelineCallCount = 0;
    mockGetContactTimeline.mockImplementation(async () => {
      timelineCallCount += 1;
      return {
        items: [mockMsg1],
        hasMore: false,
        nextCursor: null
      };
    });

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(screen.getByText('Chào shop')).toBeTruthy();
    expect(timelineCallCount).toBe(1);

    // Incoming delta
    const deltaMsg: ConversationMessage = {
      ...mockMsg1,
      id: 202,
      textContent: 'Tin nhắn delta realtime tức thì',
      occurredAt: '2026-08-31T01:00:00Z'
    };
    emitDelta({
      sequence: 1,
      conversationId: 10,
      leadId: 1,
      channel: 'zalo',
      message: deltaMsg,
      emittedAtMs: Date.now()
    });

    // Simulated real-world dual emission: invalidation with reason 'message'
    emitInvalidated({
      conversationId: 10,
      leadId: 1,
      channel: 'zalo',
      reason: 'message',
      messageId: 202
    });

    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));

    // The bubble is visible immediately
    expect(screen.getByText('Tin nhắn delta realtime tức thì')).toBeTruthy();
    // Invalidation did NOT trigger a redundant 50-message timeline IPC reload!
    expect(timelineCallCount).toBe(1);
  });

  test('22. Sequence gap triggers resumeConversationDeltas to backfill missed messages', async () => {
    mockListConversationInbox.mockImplementation(async () => [mockCustomerItem]);
    mockGetContactTimeline.mockImplementation(async () => ({
      items: [mockMsg1],
      hasMore: false,
      nextCursor: null
    }));

    const missedMsg: ConversationMessage = {
      ...mockMsg1,
      id: 301,
      textContent: 'Tin nhắn bị lỡ do gap sequence #1',
      occurredAt: '2026-08-31T01:00:01Z'
    };

    mockResumeConversationDeltas.mockImplementation(async (afterSeq: number) => {
      if (afterSeq === 1) {
        return [
          {
            sequence: 2,
            conversationId: 10,
            leadId: 1,
            channel: 'zalo',
            message: missedMsg,
            emittedAtMs: Date.now()
          }
        ];
      }
      return [];
    });

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));

    // First receive delta sequence 1
    emitDelta({
      sequence: 1,
      conversationId: 10,
      leadId: 1,
      channel: 'zalo',
      message: { ...mockMsg1, id: 300, textContent: 'Seq 1 msg' },
      emittedAtMs: Date.now()
    });
    await tick();

    // Then receive delta sequence 3 (gap detected: expected 2, got 3)
    emitDelta({
      sequence: 3,
      conversationId: 10,
      leadId: 1,
      channel: 'zalo',
      message: { ...mockMsg1, id: 302, textContent: 'Seq 3 msg' },
      emittedAtMs: Date.now()
    });

    await tick();
    await new Promise((resolve) => setTimeout(resolve, 100));
    await tick();

    // Verify resumeConversationDeltas was invoked for seq 1
    expect(mockResumeConversationDeltas).toHaveBeenCalledWith(1, 200);
    // Both seq 1, seq 2 (replayed), and seq 3 are rendered in open timeline
    const timeline = document.querySelector('[data-timeline-container]');
    expect(timeline).toBeTruthy();
    expect(within(timeline as HTMLElement).getByText('Seq 1 msg')).toBeTruthy();
    expect(within(timeline as HTMLElement).getByText('Tin nhắn bị lỡ do gap sequence #1')).toBeTruthy();
    expect(within(timeline as HTMLElement).getByText('Seq 3 msg')).toBeTruthy();
  });

  test('23. Stale snapshot arriving after delta upsert does NOT drop the new message bubble', async () => {
    mockListConversationInbox.mockImplementation(async () => [mockCustomerItem]);
    mockGetContactTimeline.mockImplementation(async () => ({
      items: [mockMsg1],
      hasMore: false,
      nextCursor: null
    }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));

    // Receive delta for message #400
    emitDelta({
      sequence: 1,
      conversationId: 10,
      leadId: 1,
      channel: 'zalo',
      message: { ...mockMsg1, id: 400, textContent: 'New delta message #400', occurredAt: '2026-08-31T01:00:10Z' },
      emittedAtMs: Date.now()
    });
    await tick();

    // Stale snapshot resolution finishes with only older messages
    emitInvalidated({
      conversationId: 10,
      leadId: 1,
      channel: 'zalo',
      reason: 'account_status',
      messageId: undefined
    });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));

    // The newly arrived delta bubble MUST remain visible (no drop!)
    expect(screen.getByText('New delta message #400')).toBeTruthy();
  });

  test('24. Duplicate delta with seq <= lastSeenSequence is safely deduplicated', async () => {
    mockListConversationInbox.mockImplementation(async () => [mockCustomerItem]);
    mockGetContactTimeline.mockImplementation(async () => ({
      items: [mockMsg1],
      hasMore: false,
      nextCursor: null
    }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));

    const duplicateDelta = {
      sequence: 5,
      conversationId: 10,
      leadId: 1,
      channel: 'zalo' as const,
      message: { ...mockMsg1, id: 501, textContent: 'Duplicate test bubble', occurredAt: '2026-08-31T01:00:20Z' },
      emittedAtMs: Date.now()
    };

    // First arrival
    emitDelta(duplicateDelta);
    await tick();

    // Duplicate arrival with same sequence
    emitDelta(duplicateDelta);
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));
    await tick();

    const timeline = document.querySelector('[data-timeline-container]');
    const bubbles = timeline ? within(timeline as HTMLElement).getAllByText('Duplicate test bubble') : [];
    expect(bubbles.length).toBe(1);
  });

  test('25. Burst message invalidations coalesce fallback and trigger single reload if any message was not pushed via delta', async () => {
    mockListConversationInbox.mockImplementation(async () => [mockCustomerItem]);
    mockGetContactTimeline.mockImplementation(async () => ({
      items: [mockMsg1],
      hasMore: false,
      nextCursor: null
    }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));
    mockGetContactTimeline.mockClear();

    // Burst of 3 invalidations for missing messages
    emitInvalidated({
      conversationId: 10,
      leadId: 1,
      channel: 'zalo',
      reason: 'message',
      messageId: 901
    });
    emitInvalidated({
      conversationId: 10,
      leadId: 1,
      channel: 'zalo',
      reason: 'message',
      messageId: 902
    });
    emitInvalidated({
      conversationId: 10,
      leadId: 1,
      channel: 'zalo',
      reason: 'message',
      messageId: 903
    });

    // Wait for the coalesced 20ms fallback timer to fire
    await new Promise((resolve) => setTimeout(resolve, 60));
    await tick();

    // Exactly 1 authoritative timeline reload should have been triggered for the burst!
    expect(mockGetContactTimeline).toHaveBeenCalledTimes(1);
  });

  test('26. High water mark with minSequence > lastSeenSequence forces full timeline refresh on lag', async () => {
    mockListConversationInbox.mockImplementation(async () => [mockCustomerItem]);
    mockGetContactTimeline.mockImplementation(async () => ({
      items: [mockMsg1],
      hasMore: false,
      nextCursor: null,
      snapshotSequence: 100
    }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));
    mockGetContactTimeline.mockClear();

    // Server high water mark shows sequence advanced far beyond retention window (minSequence = 500)
    mockGetConversationHighWaterMark.mockImplementation(async () => ({
      currentSequence: 600,
      minSequence: 500
    }));

    // Broadcast receiver reports lag event (skipped 200 events)
    emitLagged(200);
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));
    await tick();

    // Because lastSeenSequence (100) < minSequence (500), it MUST reload full timeline page!
    expect(mockGetContactTimeline).toHaveBeenCalledTimes(1);
  });

  test('27. High water mark within replay window resumes missed deltas on reconnect', async () => {
    mockListConversationInbox.mockImplementation(async () => [mockCustomerItem]);
    mockGetContactTimeline.mockImplementation(async () => ({
      items: [mockMsg1],
      hasMore: false,
      nextCursor: null,
      snapshotSequence: 100
    }));

    // Initial high water mark before reconnect is 100
    mockGetConversationHighWaterMark.mockImplementation(async () => ({
      currentSequence: 100,
      minSequence: 1
    }));

    mockResumeConversationDeltas.mockImplementation(async () => [
      {
        sequence: 105,
        conversationId: 10,
        leadId: 1,
        channel: 'zalo' as const,
        message: { ...mockMsg1, id: 777, textContent: 'Resumed delta item #777' },
        emittedAtMs: Date.now()
      }
    ]);

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));

    // Server high water mark shows sequence advanced to 105 (within minSequence = 1)
    mockGetConversationHighWaterMark.mockImplementation(async () => ({
      currentSequence: 105,
      minSequence: 1
    }));

    // Reconnect fires
    emitReconnect();
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));
    await tick();

    // Replay should have resumed deltas and rendered resumed message
    expect(mockResumeConversationDeltas).toHaveBeenCalled();
    expect(screen.getAllByText('Resumed delta item #777').length).toBeGreaterThanOrEqual(1);
  });

  test('28a. Malformed replay deserialization error falls back to authoritative snapshot reload, updates cursor, and drains newer live deltas', async () => {
    mockListConversationInbox.mockImplementation(async () => [mockCustomerItem]);
    mockGetContactTimeline.mockImplementation(async () => ({
      items: [mockMsg1],
      hasMore: false,
      nextCursor: null,
      snapshotSequence: 100
    }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));
    mockGetContactTimeline.mockClear();

    // Replay throws error (e.g. malformed JSON in sequence 102)
    mockResumeConversationDeltas.mockImplementation(async () => {
      throw new Error('FromSqlConversionFailure: Malformed JSON payload');
    });

    // Authoritative snapshot returns recovered message and snapshotSequence = 103
    const mockRecoveredMsg: ConversationMessage = {
      ...mockMsg1,
      id: 102,
      textContent: 'Recovered snapshot msg 102',
      content: { text: 'Recovered snapshot msg 102' }
    };
    mockGetContactTimeline.mockImplementation(async () => ({
      items: [mockMsg1, mockRecoveredMsg],
      hasMore: false,
      nextCursor: null,
      snapshotSequence: 103
    }));

    // Gap triggers replay attempt (seq 103)
    emitDelta({
      sequence: 103,
      conversationId: 10,
      leadId: 1,
      channel: 'zalo' as const,
      message: {
        ...mockMsg1,
        id: 999,
        textContent: 'Live delta 103 covered by snapshot',
        content: { text: 'Live delta 103 covered by snapshot' }
      },
      emittedAtMs: Date.now()
    });

    // Trailing live delta arrives during replay recovery (seq 104)
    emitDelta({
      sequence: 104,
      conversationId: 10,
      leadId: 1,
      channel: 'zalo' as const,
      message: {
        ...mockMsg1,
        id: 1000,
        textContent: 'Live trailing delta 104 newer than snapshot',
        content: { text: 'Live trailing delta 104 newer than snapshot' }
      },
      emittedAtMs: Date.now()
    });

    // Verify snapshot reload occurred
    await waitFor(() => {
      expect(mockGetContactTimeline).toHaveBeenCalled();
    });

    // Verify snapshot recovered item is rendered
    await waitFor(() => {
      expect(screen.getAllByText('Recovered snapshot msg 102').length).toBeGreaterThanOrEqual(1);
    });

    // Verify live delta 104 newer than snapshot is drained and rendered
    await waitFor(() => {
      expect(screen.getAllByText('Live trailing delta 104 newer than snapshot').length).toBeGreaterThanOrEqual(1);
    });
  });

  test('28b. Poisoned replay with rejected snapshot keeps cursor unchanged, preserves buffer, and retries until success', async () => {
    mockListConversationInbox.mockImplementation(async () => [mockCustomerItem]);
    mockGetContactTimeline.mockImplementation(async () => ({
      items: [mockMsg1],
      hasMore: false,
      nextCursor: null,
      snapshotSequence: 100
    }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));
    mockGetContactTimeline.mockClear();

    // Replay fails
    mockResumeConversationDeltas.mockImplementation(async () => {
      throw new Error('FromSqlConversionFailure: Malformed JSON');
    });

    // First snapshot attempt rejects (e.g. temporary IPC network error), subsequent retry succeeds
    const mockRecoveredMsg: ConversationMessage = {
      ...mockMsg1,
      id: 102,
      textContent: 'Retried snapshot recovered msg 102',
      content: { text: 'Retried snapshot recovered msg 102' }
    };
    mockGetContactTimeline
      .mockRejectedValueOnce(new Error('IPC Snapshot Failed'))
      .mockImplementation(async () => ({
        items: [mockMsg1, mockRecoveredMsg],
        hasMore: false,
        nextCursor: null,
        snapshotSequence: 103
      }));

    // Emit live delta with gap
    emitDelta({
      sequence: 103,
      conversationId: 10,
      leadId: 1,
      channel: 'zalo' as const,
      message: {
        ...mockMsg1,
        id: 999,
        textContent: 'Buffered delta under failure 103',
        content: { text: 'Buffered delta under failure 103' }
      },
      emittedAtMs: Date.now()
    });

    // Next live event arrives
    emitDelta({
      sequence: 104,
      conversationId: 10,
      leadId: 1,
      channel: 'zalo' as const,
      message: {
        ...mockMsg1,
        id: 1001,
        textContent: 'Buffered delta 104',
        content: { text: 'Buffered delta 104' }
      },
      emittedAtMs: Date.now()
    });

    // Wait for retry to execute and converge
    await waitFor(() => {
      expect(mockGetContactTimeline.mock.calls.length).toBeGreaterThanOrEqual(2);
    }, { timeout: 1000 });

    await waitFor(() => {
      expect(screen.getAllByText('Retried snapshot recovered msg 102').length).toBeGreaterThanOrEqual(1);
      expect(screen.getAllByText('Buffered delta 104').length).toBeGreaterThanOrEqual(1);
    });
  });

  test('28c. Conversation switch during fallback snapshot does not corrupt newly selected conversation', async () => {
    const mockCustomer2: InboxItem = {
      ...mockCustomerItem,
      key: 'lead:2',
      leadId: 2,
      title: 'Trần Thị B',
      replyTargets: [
        {
          conversationId: 20,
          channelAccountId: 1,
          channel: 'zalo',
          accountDisplayName: 'Zalo Main',
          conversationKind: 'direct',
          lastInboundAt: '2026-07-31T10:00:00Z',
          lastMessageAt: '2026-07-31T10:00:00Z'
        }
      ]
    };
    const mockMsg2: ConversationMessage = {
      ...mockMsg1,
      id: 201,
      conversationId: 20,
      senderDisplayName: 'Trần Thị B',
      textContent: 'Tin nhắn của khách B',
      content: { text: 'Tin nhắn của khách B' }
    };

    mockListConversationInbox.mockImplementation(async () => [mockCustomerItem, mockCustomer2]);
    mockGetContactTimeline.mockImplementation(async (leadId) => {
      if (leadId === 2) {
        return {
          items: [mockMsg2],
          hasMore: false,
          nextCursor: null,
          snapshotSequence: 100
        };
      }
      return {
        items: [mockMsg1],
        hasMore: false,
        nextCursor: null,
        snapshotSequence: 100
      };
    });

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));

    // Replay throws error
    mockResumeConversationDeltas.mockImplementation(async () => {
      throw new Error('FromSqlConversionFailure');
    });

    // Delayed snapshot for contact 1
    let resolveContact1: (val: any) => void;
    mockGetContactTimeline.mockImplementation(async (leadId) => {
      if (leadId === 1) {
        return new Promise((resolve) => {
          resolveContact1 = resolve;
        });
      }
      return {
        items: [mockMsg2],
        hasMore: false,
        nextCursor: null,
        snapshotSequence: 100
      };
    });

    // Gap on contact 1 triggers fallback snapshot
    emitDelta({
      sequence: 103,
      conversationId: 10,
      leadId: 1,
      channel: 'zalo' as const,
      message: {
        ...mockMsg1,
        id: 999,
        textContent: 'Delta 103',
        content: { text: 'Delta 103' }
      },
      emittedAtMs: Date.now()
    });

    await tick();

    // User switches to Contact B
    const itemB = screen.getByText('Trần Thị B');
    await fireEvent.click(itemB);
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));

    // Contact B timeline is open
    expect(screen.getAllByText('Tin nhắn của khách B').length).toBeGreaterThanOrEqual(1);

    // Now resolve the late snapshot for contact A
    if (resolveContact1!) {
      resolveContact1({
        items: [
          {
            ...mockMsg1,
            id: 102,
            textContent: 'Late snapshot for contact A',
            content: { text: 'Late snapshot for contact A' }
          }
        ],
        hasMore: false,
        nextCursor: null,
        snapshotSequence: 103
      });
    }

    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));

    // Contact B timeline must NOT be overwritten with Contact A's message
    expect(screen.queryByText('Late snapshot for contact A')).toBeNull();
    expect(screen.getAllByText('Tin nhắn của khách B').length).toBeGreaterThanOrEqual(1);
  });

  test('29. Empty authoritative snapshot response does NOT drop newer pushed delta bubble', async () => {
    mockListConversationInbox.mockImplementation(async () => [mockCustomerItem]);
    mockGetContactTimeline.mockImplementation(async () => ({
      items: [],
      hasMore: false,
      nextCursor: null,
      snapshotSequence: 10
    }));

    render(ConversationWorkspace, { props: { activeTab: 'conversation' } });
    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));

    // A delta with sequence 11 arrives (newer than snapshotSequence 10)
    emitDelta({
      sequence: 11,
      conversationId: 10,
      leadId: 1,
      channel: 'zalo' as const,
      message: {
        ...mockMsg1,
        id: 888,
        textContent: 'New bubble arriving after empty snapshot',
        content: { text: 'New bubble arriving after empty snapshot' }
      },
      emittedAtMs: Date.now()
    });

    await tick();
    await new Promise((resolve) => setTimeout(resolve, 50));

    // Another refresh resolves with empty items and snapshotSequence 10
    emitInvalidated({
      conversationId: 10,
      leadId: 1,
      channel: 'zalo' as const,
      reason: 'history_sync',
      messageId: 0
    });

    // The sequence-11 bubble MUST remain visible in timeline
    await waitFor(() => {
      expect(screen.queryAllByText('New bubble arriving after empty snapshot').length).toBeGreaterThanOrEqual(1);
    });
  });
});
