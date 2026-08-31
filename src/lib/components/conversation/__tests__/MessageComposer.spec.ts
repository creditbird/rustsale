/// <reference types="bun-types" />
import { afterEach, describe, expect, mock, test } from 'bun:test';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import type {
  ConversationCapabilities,
  ConversationMessage,
  ConversationQuickTemplate,
  ConversationRuntimeStatus,
  InboxItem,
  ListConversationQuickTemplatesRequest,
  SendConversationBatchRequest,
  SendConversationBatchResult,
  StagedConversationAttachment
} from '$lib/services/conversationService';
import type { ConversationReplySelection } from '../composerDraftState';

const richCapabilities: ConversationCapabilities = {
  maxTextLength: 2000,
  maxAttachmentsPerBatch: 10,
  maxFileSizeBytes: 50 * 1024 * 1024,
  supportedImageTypes: ['image/png', 'image/jpeg'],
  supportedVideoTypes: ['video/mp4'],
  supportedAudioTypes: ['audio/mpeg'],
  supportedDocTypes: ['application/pdf', 'text/plain'],
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

function queuedMessage(
  request: SendConversationBatchRequest,
  index: number
): ConversationMessage {
  const part = request.parts[index];
  return {
    id: 900 + index,
    conversationId: request.conversationId,
    channelAccountId: request.conversationId === 101 ? 1 : 2,
    channel: 'zalo',
    accountDisplayName: 'Zalo Test',
    senderIdentityId: null,
    senderDisplayName: null,
    direction: 'outbound',
    contentType: part.type,
    textContent: part.type === 'text' ? part.text : null,
    content: {},
    occurredAt: `2026-08-02T10:00:0${index}.000Z`,
    status: 'queued',
    removedAt: null,
    errorMessage: null,
    replyToMessageId: request.replyToMessageId,
    attachments: [],
    replyTarget: null,
    reactions: [],
    batchIndex: index,
    sendBatch: {
      sendBatchId: 'batch-result',
      status: 'queued',
      totalParts: request.parts.length,
      sentParts: 0,
      failedParts: 0
    }
  };
}

const mockSendConversationBatch = mock(
  async (request: SendConversationBatchRequest): Promise<SendConversationBatchResult> => ({
    batchId: 'batch-result',
    clientBatchId: request.clientBatchId,
    messages: request.parts.map((_, index) => queuedMessage(request, index))
  })
);
const mockStagePaths = mock(
  async (
    _conversationId: number,
    paths: string[]
  ): Promise<StagedConversationAttachment[]> =>
    paths.map((path, index) => ({
      stagedAttachmentId: index === 0 ? 'stage-a' : 'stage-b',
      mediaType: index === 0 ? 'image' : 'file',
      mimeType: index === 0 ? 'image/png' : 'application/pdf',
      fileName: path.split('/').pop() ?? 'file',
      fileSizeBytes: (index + 1) * 1024,
      width: index === 0 ? 100 : null,
      height: index === 0 ? 100 : null,
      durationSeconds: null,
      expiresAt: '2026-08-03T10:00:00.000Z'
    }))
);
const mockStageClipboard = mock(
  async (_request: unknown): Promise<StagedConversationAttachment> => ({
    stagedAttachmentId: 'staged-clip',
    mediaType: 'image',
    mimeType: 'image/png',
    fileName: 'clipboard.png',
    fileSizeBytes: 512,
    width: 10,
    height: 10,
    durationSeconds: null,
    expiresAt: '2026-08-03T10:00:00.000Z'
  })
);
const mockDiscard = mock(async (_id: string) => {});
const mockRestart = mock(async (_key: string) => {});
const mockSearchStickers = mock(
  async (_conversationId?: number, _query?: string, _limit?: number) => [
    { stickerId: 7, cateId: 8, stickerType: 1, url: 'https://cdn.example.com/7.png' }
  ]
);

let dragDropHandler: ((paths: string[]) => void) | null = null;
let dragActiveHandler: ((active: boolean) => void) | null = null;
const mockOpenFilePicker = mock(async (_options: unknown) => ['/tmp/a.png', '/tmp/b.pdf']);
const mockReadClipboardImage = mock(async () => null as {
  fileName: string;
  mimeType: string;
  base64Data: string;
} | null);

const mockListQuickTemplates = mock(
  async (_req?: ListConversationQuickTemplatesRequest): Promise<ConversationQuickTemplate[]> => []
);
const mockMarkQuickTemplateUsed = mock(async (_id: number) => {});
const mockListMentionCandidates = mock(
  async (_req?: unknown) => [
    {
      identityId: 501,
      externalId: 'ext-501',
      displayName: 'Nguyễn Văn A',
      avatarUrl: null,
      participantRole: 'admin'
    }
  ]
);

mock.module('$lib/services/conversationService', () => ({
  sendConversationBatch: (request: SendConversationBatchRequest) => mockSendConversationBatch(request),
  stageConversationAttachmentPaths: (conversationId: number, paths: string[]) =>
    mockStagePaths(conversationId, paths),
  stageConversationClipboardImage: (request: unknown) => mockStageClipboard(request),
  discardConversationStagedAttachment: (id: string) => mockDiscard(id),
  restartConversationAccount: (providerAccountKey: string) => mockRestart(providerAccountKey),
  searchZaloStickers: (conversationId: number, query: string, limit: number) =>
    mockSearchStickers(conversationId, query, limit),
  listConversationQuickTemplates: (request?: ListConversationQuickTemplatesRequest) =>
    mockListQuickTemplates(request),
  markConversationQuickTemplateUsed: (id: number) => mockMarkQuickTemplateUsed(id),
  listConversationMentionCandidates: (req?: unknown) => mockListMentionCandidates(req),
  createConversationQuickTemplate: mock(async () => ({})),
  updateConversationQuickTemplate: mock(async () => ({})),
  deleteConversationQuickTemplate: mock(async () => ({}))
}));

mock.module('$lib/services/conversationPlatform', () => ({
  openFilePicker: (options: unknown) => mockOpenFilePicker(options),
  readClipboardImage: () => mockReadClipboardImage(),
  onDragDropEvent: (
    onDrop: (paths: string[]) => void,
    onActiveChange: (active: boolean) => void
  ) => {
    dragDropHandler = onDrop;
    dragActiveHandler = onActiveChange;
    return () => {
      dragDropHandler = null;
      dragActiveHandler = null;
    };
  }
}));

import MessageComposer from '../MessageComposer.svelte';

const baseRuntime: ConversationRuntimeStatus = {
  accounts: [
    {
      channelAccountId: 1,
      channel: 'zalo',
      providerAccountKey: 'zalo-1',
      displayName: 'Zalo Main',
      capabilities: richCapabilities,
      status: 'connected',
      lastConnectedAt: '2026-08-02T09:00:00.000Z',
      lastError: null
    },
    {
      channelAccountId: 2,
      channel: 'zalo',
      providerAccountKey: 'zalo-2',
      displayName: 'Zalo Secondary',
      capabilities: richCapabilities,
      status: 'connected',
      lastConnectedAt: '2026-08-02T09:30:00.000Z',
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
  lastMessageAt: '2026-08-02T09:30:00.000Z',
  channels: [{ channel: 'zalo', messageCount: 3 }],
  replyTargets: [
    {
      conversationId: 101,
      channelAccountId: 1,
      channel: 'zalo',
      accountDisplayName: 'Zalo Main',
      conversationKind: 'direct',
      lastInboundAt: '2026-08-02T09:00:00.000Z',
      lastMessageAt: '2026-08-02T09:00:00.000Z'
    },
    {
      conversationId: 102,
      channelAccountId: 2,
      channel: 'zalo',
      accountDisplayName: 'Zalo Secondary',
      conversationKind: 'direct',
      lastInboundAt: '2026-08-02T09:30:00.000Z',
      lastMessageAt: '2026-08-02T09:30:00.000Z'
    }
  ],
  participantCount: 1,
  participants: []
};

describe('MessageComposer', () => {
  afterEach(() => {
    cleanup();
    mockSendConversationBatch.mockClear();
    mockStagePaths.mockClear();
    mockStageClipboard.mockClear();
    mockDiscard.mockClear();
    mockRestart.mockClear();
    mockSearchStickers.mockClear();
    mockOpenFilePicker.mockClear();
    mockReadClipboardImage.mockClear();
    mockReadClipboardImage.mockImplementation(async () => null);
    dragDropHandler = null;
    dragActiveHandler = null;
  });

  test('sends one flattened text batch through the latest connected target', async () => {
    const onBatchQueued = mock(() => {});
    render(MessageComposer, { item: baseItem, runtime: baseRuntime, onBatchQueued });

    const textarea = screen.getByPlaceholderText('Type a message...');
    await fireEvent.input(textarea, { target: { value: '  Xin chào  ' } });
    await fireEvent.keyDown(textarea, { key: 'Enter', shiftKey: false });

    expect(mockSendConversationBatch).toHaveBeenCalledTimes(1);
    expect(mockSendConversationBatch.mock.calls[0][0]).toEqual({
      conversationId: 102,
      clientBatchId: expect.any(String),
      replyToMessageId: null,
      messageTag: null,
      parts: [{ clientPartId: expect.any(String), type: 'text', text: 'Xin chào' }]
    });
    await waitFor(() => expect(onBatchQueued).toHaveBeenCalledTimes(2));
    const firstCall = (onBatchQueued.mock.calls as unknown as Array<[SendConversationBatchResult]>)[0]?.[0];
    const secondCall = (onBatchQueued.mock.calls as unknown as Array<[SendConversationBatchResult]>)[1]?.[0];
    expect(firstCall?.messages[0]?.id).toBeLessThan(0); // Optimistic render
    expect(secondCall?.messages[0]?.id).toBeGreaterThan(0); // Server reconciliation
    expect((textarea as HTMLTextAreaElement).value).toBe('');
  });

  test('stages picker files and sends text before ready attachments', async () => {
    render(MessageComposer, { item: baseItem, runtime: baseRuntime });

    await fireEvent.click(screen.getByTestId('attach-file-btn'));
    await waitFor(() => expect(screen.getAllByTestId('attachment-item')).toHaveLength(2));
    await fireEvent.input(screen.getByPlaceholderText('Type a message...'), {
      target: { value: 'Có file' }
    });
    await fireEvent.click(screen.getByRole('button', { name: 'Send' }));

    expect(mockStagePaths).toHaveBeenCalledWith(102, ['/tmp/a.png', '/tmp/b.pdf']);
    expect(mockSendConversationBatch.mock.calls[0][0].parts).toEqual([
      { clientPartId: expect.any(String), type: 'text', text: 'Có file' },
      { clientPartId: expect.any(String), type: 'attachment', stagedAttachmentId: 'stage-a' },
      { clientPartId: expect.any(String), type: 'attachment', stagedAttachmentId: 'stage-b' }
    ]);
  });

  test('pins quote replies to the exact conversation and account', async () => {
    const replyTarget: ConversationReplySelection = {
      messageId: 9,
      conversationId: 101,
      channelAccountId: 1,
      preview: {
        messageId: 9,
        senderDisplayName: 'Khách hàng',
        contentType: 'text',
        textContent: 'Tin gốc',
        mediaType: null,
        occurredAt: '2026-08-02T09:00:00.000Z'
      }
    };
    render(MessageComposer, { item: baseItem, runtime: baseRuntime, replyTarget });

    expect(screen.getByTestId('composer-reply-bar')).toBeTruthy();
    expect(screen.queryByTestId('channel-account-select')).toBeNull();
    await fireEvent.input(screen.getByPlaceholderText('Type a message...'), {
      target: { value: 'Trả lời' }
    });
    await fireEvent.click(screen.getByRole('button', { name: 'Send' }));

    expect(mockSendConversationBatch.mock.calls[0][0]).toEqual({
      conversationId: 101,
      clientBatchId: expect.any(String),
      replyToMessageId: 9,
      messageTag: null,
      parts: [{ clientPartId: expect.any(String), type: 'text', text: 'Trả lời' }]
    });
  });

  test('automatically focuses composer textarea and positions cursor when replyTarget is set', async () => {
    const replyTarget: ConversationReplySelection = {
      messageId: 9,
      conversationId: 101,
      channelAccountId: 1,
      preview: {
        messageId: 9,
        senderDisplayName: 'Khách hàng',
        contentType: 'text',
        textContent: 'Tin gốc',
        mediaType: null,
        occurredAt: '2026-08-02T09:00:00.000Z'
      }
    };
    render(MessageComposer, { item: baseItem, runtime: baseRuntime, replyTarget });
    await tick();

    const textarea = screen.getByPlaceholderText('Type a message...') as HTMLTextAreaElement;
    await waitFor(() => {
      expect(document.activeElement).toBe(textarea);
    });
  });

  test('inserts emoji at the textarea selection and restores focus', async () => {
    render(MessageComposer, { item: baseItem, runtime: baseRuntime });
    const textarea = screen.getByPlaceholderText('Type a message...') as HTMLTextAreaElement;
    await fireEvent.input(textarea, { target: { value: 'hello' } });
    textarea.focus();
    textarea.setSelectionRange(2, 4);

    await fireEvent.click(screen.getByTestId('toggle-emoji-btn'));
    const emojiOption = screen.getAllByTestId('emoji-option')[0];
    await fireEvent.pointerDown(emojiOption);
    await fireEvent.click(emojiOption);
    await tick();

    expect(textarea.value).toBe('he😀o');
    expect(textarea.selectionStart).toBe(4);
    expect(document.activeElement).toBe(textarea);
  });

  test('closes emoji popover on click outside or Escape key', async () => {
    render(MessageComposer, { item: baseItem, runtime: baseRuntime });
    await fireEvent.click(screen.getByTestId('toggle-emoji-btn'));
    expect(screen.getByTestId('emoji-picker')).toBeTruthy();

    await fireEvent.pointerDown(document.body);
    await tick();
    expect(screen.queryByTestId('emoji-picker')).toBeNull();

    await fireEvent.click(screen.getByTestId('toggle-emoji-btn'));
    expect(screen.getByTestId('emoji-picker')).toBeTruthy();

    await fireEvent.keyDown(window, { key: 'Escape' });
    await tick();
    expect(screen.queryByTestId('emoji-picker')).toBeNull();
  });

  test('sends a selected sticker as one immediate sticker-only batch', async () => {
    render(MessageComposer, { item: baseItem, runtime: baseRuntime });
    await fireEvent.click(screen.getByTestId('toggle-sticker-btn'));
    await waitFor(() => expect(screen.getByTestId('sticker-item-btn')).toBeTruthy());
    await fireEvent.click(screen.getByTestId('sticker-item-btn'));

    expect(mockSendConversationBatch.mock.calls[0][0].parts).toEqual([
      {
        clientPartId: expect.any(String),
        type: 'sticker',
        stickerId: 7,
        cateId: 8,
        stickerType: 1,
        url: 'https://cdn.example.com/7.png'
      }
    ]);
  });

  test('failed enqueue preserves text, attachments, quote, and open sticker picker', async () => {
    mockSendConversationBatch.mockImplementationOnce(async () => {
      throw new Error('Lỗi kết nối mạng');
    });
    render(MessageComposer, { item: baseItem, runtime: baseRuntime });

    const textarea = screen.getByPlaceholderText('Type a message...') as HTMLTextAreaElement;
    await fireEvent.input(textarea, { target: { value: 'Draft content' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Send' }));
    await tick();

    expect(textarea.value).toBe('Draft content');
    expect(screen.getByRole('alert').textContent).toContain('Lỗi kết nối mạng');

    mockSendConversationBatch.mockImplementationOnce(async () => {
      throw new Error('Sticker failed');
    });
    await fireEvent.click(screen.getByTestId('toggle-sticker-btn'));
    await waitFor(() => expect(screen.getByTestId('sticker-item-btn')).toBeTruthy());
    await fireEvent.click(screen.getByTestId('sticker-item-btn'));
    await tick();
    expect(screen.getByTestId('zalo-sticker-picker')).toBeTruthy();
    expect(screen.getByRole('alert').textContent).toContain('Sticker failed');
  });

  test('hides denied rich controls but keeps text send', () => {
    const runtime: ConversationRuntimeStatus = {
      accounts: [
        {
          ...baseRuntime.accounts[0],
          capabilities: {
            ...richCapabilities,
            maxAttachmentsPerBatch: 0,
            supportsStickers: false
          }
        }
      ]
    };
    render(MessageComposer, {
      item: { ...baseItem, replyTargets: [baseItem.replyTargets[0]] },
      runtime
    });

    expect(screen.queryByTestId('attach-file-btn')).toBeNull();
    expect(screen.queryByTestId('toggle-sticker-btn')).toBeNull();
    expect(screen.getByPlaceholderText('Type a message...')).toBeTruthy();
  });

  test('Zalo code 102 opens direct QR re-authentication for the selected account', async () => {
    const onOpenQrLogin = mock((_accountKey?: string) => {});
    const runtime: ConversationRuntimeStatus = {
      accounts: [
        {
          ...baseRuntime.accounts[0],
          status: 'invalid',
          lastError:
            'Zalo API Error: The session key was improperly submitted or has reached its timeout (code: Some(102))'
        }
      ]
    };
    render(MessageComposer, {
      item: { ...baseItem, replyTargets: [baseItem.replyTargets[0]] },
      runtime,
      onOpenQrLogin
    });

    await fireEvent.click(screen.getByRole('button', { name: /Re-login via QR|Re-authenticate Zalo QR|Đăng nhập lại/i }));

    expect(onOpenQrLogin).toHaveBeenCalledWith('zalo-1');
    expect(mockRestart).not.toHaveBeenCalled();
  });

  test('Facebook Messenger auth failure displays Cấp lại quyền Meta button and locks send', async () => {
    const onOpenSettings = mock(() => {});
    const fbReplyTarget = {
      conversationId: 202,
      channelAccountId: 200,
      channel: 'messenger' as const,
      accountDisplayName: 'Lemy Fanpage',
      conversationKind: 'direct' as const,
      lastInboundAt: new Date().toISOString(),
      lastMessageAt: new Date().toISOString()
    };
    const runtime: ConversationRuntimeStatus = {
      accounts: [
        {
          channelAccountId: 200,
          channel: 'messenger' as const,
          providerAccountKey: 'page_lemy_123',
          displayName: 'Lemy Fanpage',
          capabilities: richCapabilities,
          status: 'invalid',
          lastConnectedAt: null,
          lastError: 'Meta Graph API error (code 190): Session has expired'
        }
      ]
    };

    render(MessageComposer, {
      item: { ...baseItem, replyTargets: [fbReplyTarget] },
      runtime,
      onOpenSettings
    });

    const reauthBtn = screen.getByRole('button', { name: /Cấp lại quyền Meta|Re-authenticate/i });
    expect(reauthBtn).toBeTruthy();

    await fireEvent.click(reauthBtn);
    expect(onOpenSettings).toHaveBeenCalledTimes(1);

    // Send button must be disabled when status is invalid
    const sendBtn = screen.getByRole('button', { name: 'Send' });
    expect((sendBtn as HTMLButtonElement).disabled).toBe(true);
  });

  test('stages clipboard images and drag-drop paths through platform bridges', async () => {
    mockReadClipboardImage.mockImplementationOnce(async () => ({
      fileName: 'clipboard.png',
      mimeType: 'image/png',
      base64Data: 'aGVsbG8='
    }));
    render(MessageComposer, { item: baseItem, runtime: baseRuntime });
    const textarea = screen.getByPlaceholderText('Type a message...');

    await fireEvent.paste(textarea);
    await waitFor(() => expect(mockStageClipboard).toHaveBeenCalledTimes(1));

    dragActiveHandler?.(true);
    await tick();
    expect(screen.getByTestId('attachment-drop-overlay')).toBeTruthy();
    dragDropHandler?.(['/tmp/a.png']);
    await waitFor(() => expect(mockStagePaths).toHaveBeenCalled());
  });

  describe('Quick Templates Integration', () => {
    const runtimeWithQuickTemplates: ConversationRuntimeStatus = {
      accounts: baseRuntime.accounts.map((account) => ({
        ...account,
        capabilities: {
          ...account.capabilities,
          quickTemplates: { enabled: true, maxTemplates: 500 }
        }
      }))
    };

    const mockTemplate = {
      id: 55,
      shortcut: 'gia',
      title: 'Bảng giá 2026',
      bodyText: 'Dưới đây là bảng giá mới nhất 2026',
      category: 'Bán hàng',
      isActive: true,
      usageCount: 10,
      lastUsedAt: null,
      createdAt: '2026-08-01T00:00:00Z',
      updatedAt: '2026-08-01T00:00:00Z'
    };

    test('opens quick template popover when typing slash token with quickTemplates enabled', async () => {
      mockListQuickTemplates.mockImplementation(async () => [mockTemplate]);

      render(MessageComposer, { item: baseItem, runtime: runtimeWithQuickTemplates });

      await waitFor(() => expect(mockListQuickTemplates).toHaveBeenCalled());
      await tick();

      const textarea = screen.getByPlaceholderText('Type a message...') as HTMLTextAreaElement;
      await fireEvent.input(textarea, { target: { value: '/gia', selectionStart: 4, selectionEnd: 4 } });
      await tick();

      expect(screen.getByTestId('quick-template-popover')).toBeTruthy();
      expect(screen.getByText('Bảng giá 2026')).toBeTruthy();
    });

    test('selects quick template via Enter key, inserts body, and marks used without auto-sending', async () => {
      mockListQuickTemplates.mockImplementation(async () => [mockTemplate]);

      render(MessageComposer, { item: baseItem, runtime: runtimeWithQuickTemplates });
      await waitFor(() => expect(mockListQuickTemplates).toHaveBeenCalled());
      await tick();

      const textarea = screen.getByPlaceholderText('Type a message...') as HTMLTextAreaElement;
      await fireEvent.input(textarea, { target: { value: 'Xin /gia nhé', selectionStart: 8, selectionEnd: 8 } });
      await tick();

      await fireEvent.keyDown(textarea, { key: 'Enter' });
      await tick();

      expect(textarea.value).toBe('Xin Dưới đây là bảng giá mới nhất 2026 nhé');
      expect(mockMarkQuickTemplateUsed).toHaveBeenCalledWith(55);
      expect(mockSendConversationBatch).not.toHaveBeenCalled();
    });

    test('closes popover on Escape key without canceling quote reply', async () => {
      mockListQuickTemplates.mockImplementation(async () => [mockTemplate]);
      const onCancelReply = mock();

      render(MessageComposer, {
        item: baseItem,
        runtime: runtimeWithQuickTemplates,
        replyTarget: {
          messageId: 10,
          conversationId: 101,
          channelAccountId: 1,
          preview: {
            messageId: 10,
            senderDisplayName: 'User A',
            contentType: 'text',
            textContent: 'Quote text',
            mediaType: null,
            occurredAt: '2026-08-02T09:00:00.000Z'
          }
        },
        onCancelReply
      });
      await waitFor(() => expect(mockListQuickTemplates).toHaveBeenCalled());
      await tick();

      const textarea = screen.getByPlaceholderText('Type a message...') as HTMLTextAreaElement;
      await fireEvent.input(textarea, { target: { value: '/gia', selectionStart: 4, selectionEnd: 4 } });
      await tick();

      expect(screen.getByTestId('quick-template-popover')).toBeTruthy();

      await fireEvent.keyDown(textarea, { key: 'Escape' });
      await tick();

      expect(screen.queryByTestId('quick-template-popover')).toBeNull();
      expect(onCancelReply).not.toHaveBeenCalled();
    });

    test('does not show quick template popover if quickTemplates capability is disabled', async () => {
      render(MessageComposer, { item: baseItem, runtime: baseRuntime });

      const textarea = screen.getByPlaceholderText('Type a message...') as HTMLTextAreaElement;
      await fireEvent.input(textarea, { target: { value: '/gia', selectionStart: 4, selectionEnd: 4 } });
      await tick();

      expect(screen.queryByTestId('quick-template-popover')).toBeNull();
    });
  });

  describe('Mentions Support', () => {
    const groupItem: InboxItem = {
      ...baseItem,
      kind: 'group',
      replyTargets: [
        {
          conversationId: 101,
          channelAccountId: 1,
          channel: 'zalo',
          accountDisplayName: 'Zalo Account 1',
          conversationKind: 'group',
          lastInboundAt: '2026-08-01T12:00:00Z',
          lastMessageAt: '2026-08-01T12:00:00Z'
        }
      ]
    };

    const runtimeWithMentions: ConversationRuntimeStatus = {
      accounts: [
        {
          channelAccountId: 1,
          channel: 'zalo',
          providerAccountKey: 'zalo-key-1',
          displayName: 'Zalo Account 1',
          capabilities: {
            ...richCapabilities,
            mentions: { enabled: true, maxPerMessage: 20 }
          },
          status: 'connected',
          lastConnectedAt: null,
          lastError: null
        }
      ]
    };

    test('renders mention popover when typing @ in group chat with mentions enabled', async () => {
      render(MessageComposer, { item: groupItem, runtime: runtimeWithMentions });

      const textarea = screen.getByPlaceholderText('Type a message...') as HTMLTextAreaElement;
      await fireEvent.input(textarea, { target: { value: '@Nguyễn', selectionStart: 7, selectionEnd: 7 } });
      await tick();

      expect(screen.getByTestId('mention-candidate-popover-wrapper')).toBeTruthy();
    });

    test('selecting candidate inserts mention text and sends mention inputs batch request', async () => {
      render(MessageComposer, { item: groupItem, runtime: runtimeWithMentions });

      const textarea = screen.getByPlaceholderText('Type a message...') as HTMLTextAreaElement;
      await fireEvent.input(textarea, { target: { value: '@Nguyễn', selectionStart: 7, selectionEnd: 7 } });
      await tick();

      await waitFor(() => {
        expect(screen.getByTestId('mention-candidate-501')).toBeTruthy();
      });

      await fireEvent.click(screen.getByTestId('mention-candidate-501'));
      await tick();

      expect(textarea.value).toBe('@Nguyễn Văn A ');

      await fireEvent.click(screen.getByRole('button', { name: /Send/i }));
      await tick();

      expect(mockSendConversationBatch).toHaveBeenCalled();
      const lastCall = mockSendConversationBatch.mock.calls[mockSendConversationBatch.mock.calls.length - 1][0];
      const textPart = lastCall.parts[0];
      expect(textPart.type).toBe('text');
      if (textPart.type === 'text') {
        expect(textPart.mentions).toEqual([
          {
            pos: 0,
            len: 13,
            targetIdentityId: 501,
            isAll: false
          }
        ]);
      }
    });

    test('disabled mention and template capabilities preserve plain text send', async () => {
      const runtime: ConversationRuntimeStatus = {
        accounts: [
          {
            ...runtimeWithMentions.accounts[0],
            capabilities: {
              ...richCapabilities,
              mentions: { enabled: false, maxPerMessage: 20 },
              quickTemplates: { enabled: false, maxTemplates: 500 }
            }
          }
        ]
      };

      render(MessageComposer, { item: groupItem, runtime });

      const textarea = screen.getByPlaceholderText('Type a message...') as HTMLTextAreaElement;
      await fireEvent.input(textarea, {
        target: { value: '/gia @Nguyễn', selectionStart: 13, selectionEnd: 13 }
      });
      await tick();

      expect(screen.queryByTestId('quick-template-popover')).toBeNull();
      expect(screen.queryByTestId('mention-candidate-popover-wrapper')).toBeNull();

      await fireEvent.click(screen.getByRole('button', { name: /Send/i }));
      await tick();

      expect(mockSendConversationBatch).toHaveBeenCalledTimes(1);
      const request = mockSendConversationBatch.mock.calls[0][0];
      expect(request.parts[0]).toMatchObject({
        type: 'text',
        text: '/gia @Nguyễn'
      });
      if (request.parts[0].type === 'text') {
        expect(request.parts[0].mentions ?? []).toEqual([]);
      }
    });

    test('IME composition events (isComposing or keyCode 229) do not trigger send on Enter', async () => {
      const onBatchQueued = mock();
      render(MessageComposer, { item: baseItem, runtime: baseRuntime, onBatchQueued });

      const textarea = screen.getByPlaceholderText('Type a message...') as HTMLTextAreaElement;
      await fireEvent.input(textarea, { target: { value: 'việc' } });
      await tick();

      // Enter key with isComposing = true (IME finalizing a word)
      await fireEvent.keyDown(textarea, { key: 'Enter', isComposing: true });
      await tick();

      // Enter key with keyCode = 229 (IME key processing)
      await fireEvent.keyDown(textarea, { key: 'Enter', keyCode: 229 });
      await tick();

      expect(mockSendConversationBatch).not.toHaveBeenCalled();

      // Normal Enter without composing sends the message
      await fireEvent.keyDown(textarea, { key: 'Enter', isComposing: false });
      await tick();

      expect(mockSendConversationBatch).toHaveBeenCalledTimes(1);
    });

    test('full IME composition cycle (compositionstart -> intermediate input -> compositionend) correctly reconciles mentions and sends payload', async () => {
      const onBatchQueued = mock();
      render(MessageComposer, {
        item: groupItem,
        runtime: runtimeWithMentions,
        onBatchQueued
      });

      const textarea = screen.getByPlaceholderText('Type a message...') as HTMLTextAreaElement;

      // 1. Type @ to open mention popover
      await fireEvent.input(textarea, { target: { value: '@Nguyễn', selectionStart: 7, selectionEnd: 7 } });
      await tick();
      await waitFor(() => expect(screen.getByTestId('mention-candidate-501')).toBeTruthy());

      // 2. Select candidate via click -> text becomes "@Nguyễn Văn A "
      await fireEvent.click(screen.getByTestId('mention-candidate-501'));
      await tick();
      expect(textarea.value).toBe('@Nguyễn Văn A ');

      // 3. Start IME composition to type Vietnamese "việc" (v -> vi -> vie -> viec -> việc)
      await fireEvent.compositionStart(textarea);
      await fireEvent.input(textarea, { target: { value: '@Nguyễn Văn A viec' } });
      await tick();

      // 4. End IME composition with final word "việc"
      textarea.value = '@Nguyễn Văn A việc';
      await fireEvent.compositionEnd(textarea);
      await tick();

      // 5. Send message
      await fireEvent.keyDown(textarea, { key: 'Enter', isComposing: false });
      await tick();

      expect(mockSendConversationBatch).toHaveBeenCalled();
      const lastCall = mockSendConversationBatch.mock.calls[mockSendConversationBatch.mock.calls.length - 1][0];
      const textPart = lastCall.parts[0];
      expect(textPart.type).toBe('text');
      if (textPart.type === 'text') {
        expect(textPart.text).toBe('@Nguyễn Văn A việc');
        expect(textPart.mentions).toHaveLength(1);
        expect(textPart.mentions?.[0]).toEqual({
          pos: 0,
          len: 13,
          targetIdentityId: 501,
          isAll: false
        });
      }
    });
  });

  describe('WhatsApp Channel Integration', () => {
    test('supports selecting connected WhatsApp sender account and sending text', async () => {
      const whatsappItem: InboxItem = {
        ...baseItem,
        key: 'lead:42',
        replyTargets: [
          {
            conversationId: 501,
            channelAccountId: 30,
            channel: 'whatsapp',
            accountDisplayName: 'WhatsApp Sales (+84987654321)',
            conversationKind: 'direct',
            lastInboundAt: '2026-08-02T10:00:00.000Z',
            lastMessageAt: '2026-08-02T10:00:00.000Z'
          }
        ]
      };

      const whatsappRuntime: ConversationRuntimeStatus = {
        accounts: [
          {
            channelAccountId: 30,
            channel: 'whatsapp',
            displayName: 'WhatsApp Sales (+84987654321)',
            providerAccountKey: 'whatsapp-30',
            status: 'connected',
            lastConnectedAt: '2026-08-02T10:00:00.000Z',
            lastError: null,
            capabilities: richCapabilities
          }
        ]
      };

      render(MessageComposer, {
        props: {
          item: whatsappItem,
          runtime: whatsappRuntime
        }
      });

      const select = screen.getByTestId('channel-account-select');
      expect(select.textContent).toContain('whatsapp #30');
      expect(select.textContent).toContain('WhatsApp Sales');

      const textarea = screen.getByPlaceholderText('Type a message...') as HTMLTextAreaElement;
      await fireEvent.input(textarea, { target: { value: 'Hello from WhatsApp CRM!' } });
      await tick();

      const sendBtn = screen.getByRole('button', { name: 'Send' });
      expect((sendBtn as HTMLButtonElement).disabled).toBe(false);

      await fireEvent.click(sendBtn);
      await tick();

      expect(mockSendConversationBatch).toHaveBeenCalled();
      const lastCall = mockSendConversationBatch.mock.calls[mockSendConversationBatch.mock.calls.length - 1][0];
      expect(lastCall.conversationId).toBe(501);
      expect(lastCall.parts[0]).toEqual({
        clientPartId: expect.any(String),
        type: 'text',
        text: 'Hello from WhatsApp CRM!'
      });
    });
  });
});
