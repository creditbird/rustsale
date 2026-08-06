import { describe, expect, test, mock, beforeEach, afterEach } from 'bun:test';
import { render, fireEvent, screen, cleanup } from '@testing-library/svelte';
import ForwardConversationDialog from '../ForwardConversationDialog.svelte';
import type { ConversationMessage, InboxItem } from '$lib/services/conversationService';

const mockForwardConversationMessage = mock();
const mockGetConversationForwardOperation = mock();
const mockRetryConversationForwardTarget = mock();

mock.module('$lib/services/conversationService', () => ({
  forwardConversationMessage: mockForwardConversationMessage,
  getConversationForwardOperation: mockGetConversationForwardOperation,
  retryConversationForwardTarget: mockRetryConversationForwardTarget
}));

describe('ForwardConversationDialog', () => {
  const mockSourceMessage: ConversationMessage = {
    id: 999,
    conversationId: 10,
    channelAccountId: 1,
    channel: 'zalo',
    accountDisplayName: 'Account 1',
    senderIdentityId: 100,
    senderDisplayName: 'Sender A',
    direction: 'inbound',
    contentType: 'text',
    textContent: 'Hello, this is a test message to forward',
    content: {},
    occurredAt: '2026-08-01T10:00:00Z',
    status: 'sent',
    removedAt: null,
    errorMessage: null,
    replyToMessageId: null,
    attachments: [],
    replyTarget: null,
    reactions: [],
    batchIndex: null,
    sendBatch: null
  };

  const mockInbox: InboxItem[] = [
    {
      key: 'c1',
      kind: 'customer',
      leadId: 1,
      conversationId: 10,
      identityId: 100,
      title: 'Sender A (Source Conv)',
      avatarUrl: null,
      lastMessagePreview: 'Hi',
      lastMessageAt: '2026-08-01T10:00:00Z',
      channels: [{ channel: 'zalo', messageCount: 1 }],
      replyTargets: [
        {
          conversationId: 10,
          channelAccountId: 1,
          channel: 'zalo',
          accountDisplayName: 'Account 1',
          conversationKind: 'direct',
          lastInboundAt: null,
          lastMessageAt: null
        }
      ],
      participantCount: 1,
      participants: []
    },
    {
      key: 'c2',
      kind: 'customer',
      leadId: 2,
      conversationId: 20,
      identityId: 200,
      title: 'Target Customer B',
      avatarUrl: null,
      lastMessagePreview: 'Hello',
      lastMessageAt: '2026-08-01T11:00:00Z',
      channels: [{ channel: 'zalo', messageCount: 2 }],
      replyTargets: [
        {
          conversationId: 20,
          channelAccountId: 1,
          channel: 'zalo',
          accountDisplayName: 'Account 1',
          conversationKind: 'direct',
          lastInboundAt: null,
          lastMessageAt: null
        }
      ],
      participantCount: 1,
      participants: []
    },
    {
      key: 'c3',
      kind: 'group',
      leadId: null,
      conversationId: 30,
      identityId: null,
      title: 'Target Support Group C',
      avatarUrl: null,
      lastMessagePreview: 'OK',
      lastMessageAt: '2026-08-01T12:00:00Z',
      channels: [{ channel: 'zalo', messageCount: 5 }],
      replyTargets: [
        {
          conversationId: 30,
          channelAccountId: 1,
          channel: 'zalo',
          accountDisplayName: 'Account 1',
          conversationKind: 'group',
          lastInboundAt: null,
          lastMessageAt: null
        }
      ],
      participantCount: 3,
      participants: []
    }
  ];

  afterEach(() => {
    cleanup();
    document.body.innerHTML = '';
  });

  beforeEach(() => {
    mockForwardConversationMessage.mockReset();
    mockGetConversationForwardOperation.mockReset();
    mockRetryConversationForwardTarget.mockReset();
  });

  test('renders dialog when open and shows source preview and eligible target conversations', () => {
    render(ForwardConversationDialog, {
      props: {
        open: true,
        sourceMessage: mockSourceMessage,
        inbox: mockInbox
      }
    });

    expect(screen.getByTestId('forward-conversation-dialog')).toBeTruthy();
    expect(screen.getByText('Chuyển tiếp tin nhắn')).toBeTruthy();
    expect(screen.getByText('Hello, this is a test message to forward')).toBeTruthy();

    // Source conversation #10 should be excluded, target #20 and #30 should be visible
    expect(screen.getByText('Target Customer B')).toBeTruthy();
    expect(screen.getByText('Target Support Group C')).toBeTruthy();
    expect(screen.queryByText('Sender A (Source Conv)')).toBeNull();
  });

  test('filters target conversations by search query', async () => {
    render(ForwardConversationDialog, {
      props: {
        open: true,
        sourceMessage: mockSourceMessage,
        inbox: mockInbox
      }
    });

    const searchInput = screen.getByTestId('forward-dialog-search-input');
    await fireEvent.input(searchInput, { target: { value: 'Group' } });

    expect(screen.getByText('Target Support Group C')).toBeTruthy();
    expect(screen.queryByText('Target Customer B')).toBeNull();
  });

  test('submits forward request and renders target result list', async () => {
    mockForwardConversationMessage.mockResolvedValueOnce({
      clientOperationId: 'op-test-1',
      sourceMessageId: 999,
      status: 'partial',
      targets: [
        {
          targetConversationId: 20,
          clientBatchId: 'b-20',
          status: 'sent',
          errorCode: null,
          errorMessage: null
        },
        {
          targetConversationId: 30,
          clientBatchId: 'b-30',
          status: 'failed',
          errorCode: 'FAIL_SEND',
          errorMessage: 'Lỗi gửi tin'
        }
      ]
    });

    const onForwarded = mock();
    render(ForwardConversationDialog, {
      props: {
        open: true,
        sourceMessage: mockSourceMessage,
        inbox: mockInbox,
        onForwarded
      }
    });

    // Select target 20 & target 30
    const target20 = screen.getByTestId('forward-target-item-20');
    const checkbox20 = target20.querySelector('input[type="checkbox"]')!;
    await fireEvent.click(checkbox20);

    const target30 = screen.getByTestId('forward-target-item-30');
    const checkbox30 = target30.querySelector('input[type="checkbox"]')!;
    await fireEvent.click(checkbox30);

    const submitBtn = screen.getByTestId('submit-forward-button');
    await fireEvent.click(submitBtn);

    expect(mockForwardConversationMessage).toHaveBeenCalledTimes(1);
    const callArg = mockForwardConversationMessage.mock.calls[0][0];
    expect(callArg.sourceMessageId).toBe(999);
    expect(callArg.targetConversationIds).toEqual([20, 30]);
    expect(typeof callArg.clientOperationId).toBe('string');

    // Should display target result list
    expect(await screen.findByTestId('forward-target-result-list')).toBeTruthy();
    expect(screen.getByTestId('forward-target-result-20')).toBeTruthy();
    expect(screen.getByTestId('forward-target-result-30')).toBeTruthy();
    expect(screen.getByText('Lỗi gửi tin')).toBeTruthy();
    expect(onForwarded).toHaveBeenCalledTimes(1);
  });

  test('polls queued operation until target reaches terminal status', async () => {
    mockForwardConversationMessage.mockResolvedValueOnce({
      clientOperationId: 'op-test-poll',
      sourceMessageId: 999,
      status: 'queued',
      targets: [
        {
          targetConversationId: 20,
          clientBatchId: 'b-20',
          status: 'queued',
          errorCode: null,
          errorMessage: null
        }
      ]
    });
    mockGetConversationForwardOperation.mockResolvedValueOnce({
      clientOperationId: 'op-test-poll',
      sourceMessageId: 999,
      status: 'sent',
      targets: [
        {
          targetConversationId: 20,
          clientBatchId: 'b-20',
          status: 'sent',
          errorCode: null,
          errorMessage: null
        }
      ]
    });

    render(ForwardConversationDialog, {
      props: {
        open: true,
        sourceMessage: mockSourceMessage,
        inbox: mockInbox
      }
    });

    const target20 = screen.getByTestId('forward-target-item-20');
    const checkbox20 = target20.querySelector('input[type="checkbox"]')!;
    await fireEvent.click(checkbox20);
    await fireEvent.click(screen.getByTestId('submit-forward-button'));

    expect(await screen.findByText('Đã gửi', {}, { timeout: 2000 })).toBeTruthy();
    expect(mockGetConversationForwardOperation).toHaveBeenCalledWith('op-test-poll');
  });

  test('retries failed target when retry button clicked', async () => {
    mockForwardConversationMessage.mockResolvedValueOnce({
      clientOperationId: 'op-test-2',
      sourceMessageId: 999,
      status: 'failed',
      targets: [
        {
          targetConversationId: 30,
          clientBatchId: 'b-30',
          status: 'failed',
          errorCode: 'FAIL_SEND',
          errorMessage: 'Lỗi gửi tin'
        }
      ]
    });

    mockRetryConversationForwardTarget.mockResolvedValueOnce({
      targetConversationId: 30,
      clientBatchId: 'b-30-retry',
      status: 'sent',
      errorCode: null,
      errorMessage: null
    });

    render(ForwardConversationDialog, {
      props: {
        open: true,
        sourceMessage: mockSourceMessage,
        inbox: mockInbox
      }
    });

    // Select target 30 and submit
    const target30 = screen.getByTestId('forward-target-item-30');
    const checkbox30 = target30.querySelector('input[type="checkbox"]')!;
    await fireEvent.click(checkbox30);

    const submitBtn = screen.getByTestId('submit-forward-button');
    await fireEvent.click(submitBtn);

    // Wait for retry button
    const retryBtn = await screen.findByTestId('retry-forward-target-30');
    await fireEvent.click(retryBtn);

    expect(mockRetryConversationForwardTarget).toHaveBeenCalledTimes(1);
    expect(mockRetryConversationForwardTarget.mock.calls[0][0]).toMatchObject({
      clientOperationId: 'op-test-2',
      targetConversationId: 30
    });
  });
});
