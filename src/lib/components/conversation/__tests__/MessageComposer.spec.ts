/// <reference types="bun-types" />
import { afterEach, describe, expect, test, mock } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import type { InboxItem, ConversationRuntimeStatus, ConversationMessage } from '$lib/services/conversationService';
import MessageComposer from '../MessageComposer.svelte';

const mockSendConversationMessage = mock(async (request: any): Promise<ConversationMessage> => {
  return {
    id: 999,
    conversationId: request.conversationId,
    channelAccountId: request.channelAccountId,
    channel: 'zalo',
    accountDisplayName: 'Zalo Main',
    senderIdentityId: 1,
    senderDisplayName: 'Agent',
    senderType: 'agent',
    senderName: 'Agent',
    senderAvatar: null,
    direction: 'outbound',
    providerMessageId: 'p-999',
    contentType: 'text',
    textContent: request.text,
    content: { text: request.text },
    occurredAt: '2026-07-31T12:00:00Z',
    sentAt: '2026-07-31T12:00:00Z',
    status: 'sent',
    removedAt: null,
    errorMessage: null,
    replyToMessageId: null
  } as unknown as ConversationMessage;
});
const mockRestartConversationAccount = mock(async (_providerAccountKey: string) => {});

mock.module('$lib/services/conversationService', () => ({
  sendConversationMessage: (req: any) => mockSendConversationMessage(req),
  restartConversationAccount: (providerAccountKey: string) =>
    mockRestartConversationAccount(providerAccountKey)
}));

describe('MessageComposer Component', () => {
  afterEach(() => {
    cleanup();
    document.body.innerHTML = '';
    mockSendConversationMessage.mockClear();
    mockRestartConversationAccount.mockClear();
  });

  const baseRuntime: ConversationRuntimeStatus = {
    accounts: [
      {
        channelAccountId: 1,
        channel: 'zalo',
        providerAccountKey: 'zalo-1',
        displayName: 'Zalo Main',
        status: 'connected',
        lastConnectedAt: '2026-07-31T10:00:00Z',
        lastError: null
      },
      {
        channelAccountId: 2,
        channel: 'zalo',
        providerAccountKey: 'zalo-2',
        displayName: 'Zalo Secondary',
        status: 'connected',
        lastConnectedAt: '2026-07-31T11:00:00Z',
        lastError: null
      }
    ]
  };

  const baseItem: InboxItem = {
    key: 'item-101',
    kind: 'customer',
    leadId: 10,
    conversationId: null,
    identityId: 201,
    title: 'Nguyễn Văn A',
    avatarUrl: null,
    lastMessagePreview: 'Xin chào',
    lastMessageAt: '2026-07-31T11:00:00Z',
    channels: [{ channel: 'zalo', messageCount: 3 }],
    replyTargets: [
      {
        conversationId: 101,
        channelAccountId: 1,
        channel: 'zalo',
        accountDisplayName: 'Zalo Main',
        conversationKind: 'direct',
        lastInboundAt: '2026-07-31T10:00:00Z',
        lastMessageAt: '2026-07-31T10:00:00Z'
      },
      {
        conversationId: 102,
        channelAccountId: 2,
        channel: 'zalo',
        accountDisplayName: 'Zalo Secondary',
        conversationKind: 'direct',
        lastInboundAt: '2026-07-31T11:00:00Z',
        lastMessageAt: '2026-07-31T11:00:00Z'
      }
    ],
    participantCount: 1,
    participants: []
  };

  test('direct customer defaults to connected reply target with latest lastInboundAt', async () => {
    const onQueued = mock(() => {});
    render(MessageComposer, {
      props: {
        item: baseItem,
        runtime: baseRuntime,
        onQueued
      }
    });

    const textarea = screen.getByPlaceholderText(/Nhập tin nhắn|Send message/i);
    await fireEvent.input(textarea, { target: { value: 'Xin chào' } });

    const sendBtn = screen.getByRole('button', { name: /Gửi|Send/i });
    await fireEvent.click(sendBtn);

    expect(mockSendConversationMessage).toHaveBeenCalledTimes(1);
    expect(mockSendConversationMessage).toHaveBeenCalledWith({
      conversationId: 102,
      channelAccountId: 2,
      text: 'Xin chào',
      replyToMessageId: null
    });
  });

  test('manually selected account is not replaced when runtime status refreshes', async () => {
    const onQueued = mock(() => {});
    const { rerender } = render(MessageComposer, {
      props: {
        item: baseItem,
        runtime: baseRuntime,
        onQueued
      }
    });

    // Manually select account 1 via Select.Trigger interaction
    const trigger = screen.getByTestId('channel-account-select');
    await fireEvent.keyDown(trigger, { key: 'ArrowDown' });
    await tick();

    const option1 = await screen.findByTestId('account-option-1');
    await fireEvent(option1, new PointerEvent('pointerdown', { bubbles: true, cancelable: true, pointerId: 1, pointerType: 'mouse' }));
    await fireEvent(option1, new PointerEvent('pointerup', { bubbles: true, cancelable: true, pointerId: 1, pointerType: 'mouse' }));
    await fireEvent(option1, new MouseEvent('click', { bubbles: true, cancelable: true }));
    await tick();

    // Refresh runtime status where account 2 has a newer timestamp
    const updatedRuntime: ConversationRuntimeStatus = {
      accounts: [
        { ...baseRuntime.accounts[0], lastConnectedAt: '2026-07-31T15:00:00Z' },
        { ...baseRuntime.accounts[1], lastConnectedAt: '2026-07-31T16:00:00Z' }
      ]
    };

    const updatedItem: InboxItem = {
      ...baseItem,
      replyTargets: [
        { ...baseItem.replyTargets[0], lastInboundAt: '2026-07-31T10:00:00Z' },
        { ...baseItem.replyTargets[1], lastInboundAt: '2026-07-31T16:00:00Z' }
      ]
    };

    await rerender({ item: updatedItem, runtime: updatedRuntime, onQueued });
    await tick();

    const textarea = screen.getByPlaceholderText(/Nhập tin nhắn|Send message/i);
    await fireEvent.input(textarea, { target: { value: 'Tin nhắn thủ công' } });

    const sendBtn = screen.getByRole('button', { name: /Gửi|Send/i });
    await fireEvent.click(sendBtn);

    expect(mockSendConversationMessage).toHaveBeenCalledWith({
      conversationId: 101,
      channelAccountId: 1,
      text: 'Tin nhắn thủ công',
      replyToMessageId: null
    });
  });

  test('group mode exposes one locked Zalo target without channel switcher', () => {
    const groupItem: InboxItem = {
      ...baseItem,
      kind: 'group',
      replyTargets: [
        {
          conversationId: 101,
          channelAccountId: 1,
          channel: 'zalo',
          accountDisplayName: 'Zalo Main',
          conversationKind: 'group',
          lastInboundAt: '2026-07-31T10:00:00Z',
          lastMessageAt: '2026-07-31T10:00:00Z'
        }
      ]
    };

    render(MessageComposer, {
      props: {
        item: groupItem,
        runtime: baseRuntime,
        onQueued: () => {}
      }
    });

    expect(screen.queryByTestId('channel-account-select')).toBeNull();
    expect(screen.getByTestId('locked-zalo-target')).toBeTruthy();
  });

  test('disconnected/connecting/degraded/invalid accounts show a visible reason and cannot send', async () => {
    const degradedRuntime: ConversationRuntimeStatus = {
      accounts: [
        {
          channelAccountId: 1,
          channel: 'zalo',
          providerAccountKey: 'zalo-1',
          displayName: 'Zalo Main',
          status: 'disconnected',
          lastConnectedAt: null,
          lastError: 'Tài khoản chưa đăng nhập'
        }
      ]
    };

    const zaloOnlyItem: InboxItem = {
      ...baseItem,
      replyTargets: [
        {
          conversationId: 101,
          channelAccountId: 1,
          channel: 'zalo',
          accountDisplayName: 'Zalo Main',
          conversationKind: 'direct',
          lastInboundAt: '2026-07-31T10:00:00Z',
          lastMessageAt: '2026-07-31T10:00:00Z'
        }
      ]
    };

    render(MessageComposer, {
      props: {
        item: zaloOnlyItem,
        runtime: degradedRuntime,
        onQueued: () => {}
      }
    });

    const alert = screen.getByRole('alert');
    expect(alert).toBeTruthy();
    expect(alert.textContent).toContain('Tài khoản chưa đăng nhập');

    const sendBtn = screen.getByRole('button', { name: /Gửi|Send/i }) as HTMLButtonElement;
    expect(sendBtn.disabled).toBe(true);
  });

  test('disconnected account offers reconnect and Settings recovery actions', async () => {
    const onOpenSettings = mock(() => {});
    const onRuntimeRefresh = mock(() => {});
    const disconnectedRuntime: ConversationRuntimeStatus = {
      accounts: [
        {
          ...baseRuntime.accounts[0],
          status: 'disconnected',
          lastError: 'Listener stopped'
        }
      ]
    };
    const item: InboxItem = {
      ...baseItem,
      replyTargets: [baseItem.replyTargets[0]]
    };

    render(MessageComposer, {
      props: { item, runtime: disconnectedRuntime, onOpenSettings, onRuntimeRefresh }
    });

    await fireEvent.click(screen.getByRole('button', { name: /Reconnect Zalo Main/i }));

    expect(mockRestartConversationAccount).toHaveBeenCalledWith('zalo-1');
    expect(onRuntimeRefresh).toHaveBeenCalledTimes(1);

    await fireEvent.click(screen.getByRole('button', { name: /Open account settings/i }));
    expect(onOpenSettings).toHaveBeenCalledTimes(1);
  });

  test('Enter sends trimmed text, Shift+Enter inserts a newline', async () => {
    const onQueued = mock(() => {});
    render(MessageComposer, {
      props: {
        item: baseItem,
        runtime: baseRuntime,
        onQueued
      }
    });

    const textarea = screen.getByPlaceholderText(/Nhập tin nhắn|Send message/i) as HTMLTextAreaElement;

    // Shift+Enter should NOT send
    await fireEvent.keyDown(textarea, { key: 'Enter', shiftKey: true });
    expect(mockSendConversationMessage).not.toHaveBeenCalled();

    // Enter with text sends
    await fireEvent.input(textarea, { target: { value: '  Nội dung tin nhắn  ' } });
    await fireEvent.keyDown(textarea, { key: 'Enter', shiftKey: false });

    expect(mockSendConversationMessage).toHaveBeenCalledWith({
      conversationId: 102,
      channelAccountId: 2,
      text: 'Nội dung tin nhắn',
      replyToMessageId: null
    });
  });

  test('returned queued message clears textarea and calls onQueued(message)', async () => {
    const onQueued = mock(() => {});
    render(MessageComposer, {
      props: {
        item: baseItem,
        runtime: baseRuntime,
        onQueued
      }
    });

    const textarea = screen.getByPlaceholderText(/Nhập tin nhắn|Send message/i) as HTMLTextAreaElement;
    await fireEvent.input(textarea, { target: { value: 'Test message' } });

    const sendBtn = screen.getByRole('button', { name: /Gửi|Send/i });
    await fireEvent.click(sendBtn);
    await tick();

    expect(textarea.value).toBe('');
    expect(onQueued).toHaveBeenCalledTimes(1);
    expect(((onQueued.mock.calls as any[])[0][0] as ConversationMessage).id).toBe(999);
  });

  test('send failure preserves draft and exposes error string', async () => {
    mockSendConversationMessage.mockImplementationOnce(async () => {
      throw new Error('Lỗi kết nối mạng');
    });

    render(MessageComposer, {
      props: {
        item: baseItem,
        runtime: baseRuntime,
        onQueued: () => {}
      }
    });

    const textarea = screen.getByPlaceholderText(/Nhập tin nhắn|Send message/i) as HTMLTextAreaElement;
    await fireEvent.input(textarea, { target: { value: 'Draft content' } });

    const sendBtn = screen.getByRole('button', { name: /Gửi|Send/i });
    await fireEvent.click(sendBtn);
    await tick();

    // Draft preserved
    expect(textarea.value).toBe('Draft content');

    // Error message exposed
    const alert = screen.getByRole('alert');
    expect(alert.textContent).toContain('Lỗi kết nối mạng');
  });

  test('Customer item with conversationId: null sends using selected target conversationId', async () => {
    const onQueued = mock(() => {});
    const customerItemWithoutDirectId: InboxItem = {
      ...baseItem,
      conversationId: null,
      replyTargets: [
        {
          conversationId: 555,
          channelAccountId: 1,
          channel: 'zalo',
          accountDisplayName: 'Zalo Main',
          conversationKind: 'direct',
          lastInboundAt: '2026-07-31T12:00:00Z',
          lastMessageAt: '2026-07-31T12:00:00Z'
        }
      ]
    };

    render(MessageComposer, {
      props: {
        item: customerItemWithoutDirectId,
        runtime: baseRuntime,
        onQueued
      }
    });

    const textarea = screen.getByPlaceholderText(/Nhập tin nhắn|Send message/i);
    await fireEvent.input(textarea, { target: { value: 'Tin nhắn cho Customer' } });

    const sendBtn = screen.getByRole('button', { name: /Gửi|Send/i });
    expect((sendBtn as HTMLButtonElement).disabled).toBe(false);

    await fireEvent.click(sendBtn);

    expect(mockSendConversationMessage).toHaveBeenCalledWith({
      conversationId: 555,
      channelAccountId: 1,
      text: 'Tin nhắn cho Customer',
      replyToMessageId: null
    });
  });
});
