/// <reference types="bun-types" />
import { afterEach, describe, expect, mock, test } from 'bun:test';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import type {
  ConversationCapabilities,
  ConversationMessage,
  ConversationRuntimeStatus,
  InboxItem,
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
  supportsReactions: true
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
    providerMessageId: null,
    providerClientMessageId: `client-${index}`,
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
const mockStagePaths = mock(async (
  _conversationId: number,
  _paths: string[]
): Promise<StagedConversationAttachment[]> => [
  {
    stagedAttachmentId: 'stage-a',
    mediaType: 'image',
    mimeType: 'image/png',
    fileName: 'a.png',
    fileSizeBytes: 1024,
    width: 100,
    height: 100,
    durationSeconds: null,
    expiresAt: '2026-08-03T10:00:00.000Z'
  },
  {
    stagedAttachmentId: 'stage-b',
    mediaType: 'file',
    mimeType: 'application/pdf',
    fileName: 'b.pdf',
    fileSizeBytes: 2048,
    width: null,
    height: null,
    durationSeconds: null,
    expiresAt: '2026-08-03T10:00:00.000Z'
  }
]);
const mockStageClipboard = mock(async (_request: unknown): Promise<StagedConversationAttachment> => ({
  stagedAttachmentId: 'stage-clipboard',
  mediaType: 'image',
  mimeType: 'image/png',
  fileName: 'clipboard.png',
  fileSizeBytes: 128,
  width: 10,
  height: 10,
  durationSeconds: null,
  expiresAt: '2026-08-03T10:00:00.000Z'
}));
const mockDiscard = mock(async (_id: string) => {});
const mockRestart = mock(async (_providerAccountKey: string) => {});
const mockSearchStickers = mock(async (_conversationId: number, _query: string, _limit: number) => [
  { stickerId: 7, cateId: 8, stickerType: 1, url: 'https://cdn.example.com/7.png' }
]);

let dragDropHandler: ((paths: string[]) => void) | null = null;
let dragActiveHandler: ((active: boolean) => void) | null = null;
const mockOpenFilePicker = mock(async (_options: unknown) => ['/tmp/a.png', '/tmp/b.pdf']);
const mockReadClipboardImage = mock(async () => null as {
  fileName: string;
  mimeType: string;
  base64Data: string;
} | null);

mock.module('$lib/services/conversationService', () => ({
  sendConversationBatch: (request: SendConversationBatchRequest) => mockSendConversationBatch(request),
  stageConversationAttachmentPaths: (conversationId: number, paths: string[]) =>
    mockStagePaths(conversationId, paths),
  stageConversationClipboardImage: (request: unknown) => mockStageClipboard(request),
  discardConversationStagedAttachment: (id: string) => mockDiscard(id),
  restartConversationAccount: (providerAccountKey: string) => mockRestart(providerAccountKey),
  searchZaloStickers: (conversationId: number, query: string, limit: number) =>
    mockSearchStickers(conversationId, query, limit)
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

    const textarea = screen.getByPlaceholderText('Nhập tin nhắn...');
    await fireEvent.input(textarea, { target: { value: '  Xin chào  ' } });
    await fireEvent.keyDown(textarea, { key: 'Enter', shiftKey: false });

    expect(mockSendConversationBatch).toHaveBeenCalledTimes(1);
    expect(mockSendConversationBatch.mock.calls[0][0]).toEqual({
      conversationId: 102,
      clientBatchId: expect.any(String),
      replyToMessageId: null,
      parts: [{ clientPartId: expect.any(String), type: 'text', text: 'Xin chào' }]
    });
    await waitFor(() => expect(onBatchQueued).toHaveBeenCalledTimes(1));
    expect((textarea as HTMLTextAreaElement).value).toBe('');
  });

  test('stages picker files and sends text before ready attachments', async () => {
    render(MessageComposer, { item: baseItem, runtime: baseRuntime });

    await fireEvent.click(screen.getByTestId('attach-file-btn'));
    await waitFor(() => expect(screen.getAllByTestId('attachment-item')).toHaveLength(2));
    await fireEvent.input(screen.getByPlaceholderText('Nhập tin nhắn...'), {
      target: { value: 'Có file' }
    });
    await fireEvent.click(screen.getByRole('button', { name: 'Gửi' }));

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
        providerMessageId: 'provider-9',
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
    await fireEvent.input(screen.getByPlaceholderText('Nhập tin nhắn...'), {
      target: { value: 'Trả lời' }
    });
    await fireEvent.click(screen.getByRole('button', { name: 'Gửi' }));

    expect(mockSendConversationBatch.mock.calls[0][0]).toEqual({
      conversationId: 101,
      clientBatchId: expect.any(String),
      replyToMessageId: 9,
      parts: [{ clientPartId: expect.any(String), type: 'text', text: 'Trả lời' }]
    });
  });

  test('inserts emoji at the textarea selection and restores focus', async () => {
    render(MessageComposer, { item: baseItem, runtime: baseRuntime });
    const textarea = screen.getByPlaceholderText('Nhập tin nhắn...') as HTMLTextAreaElement;
    await fireEvent.input(textarea, { target: { value: 'hello' } });
    textarea.focus();
    textarea.setSelectionRange(2, 4);

    await fireEvent.click(screen.getByTestId('toggle-emoji-btn'));
    await fireEvent.click(screen.getAllByTestId('emoji-option')[0]);
    await tick();

    expect(textarea.value).toBe('he😀o');
    expect(textarea.selectionStart).toBe(4);
    expect(document.activeElement).toBe(textarea);
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
        stickerType: 1
      }
    ]);
  });

  test('failed enqueue preserves text, attachments, quote, and open sticker picker', async () => {
    mockSendConversationBatch.mockImplementationOnce(async () => {
      throw new Error('Lỗi kết nối mạng');
    });
    render(MessageComposer, { item: baseItem, runtime: baseRuntime });

    const textarea = screen.getByPlaceholderText('Nhập tin nhắn...') as HTMLTextAreaElement;
    await fireEvent.input(textarea, { target: { value: 'Draft content' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Gửi' }));
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
    expect(screen.getByPlaceholderText('Nhập tin nhắn...')).toBeTruthy();
  });

  test('stages clipboard images and drag-drop paths through platform bridges', async () => {
    mockReadClipboardImage.mockImplementationOnce(async () => ({
      fileName: 'clipboard.png',
      mimeType: 'image/png',
      base64Data: 'aGVsbG8='
    }));
    render(MessageComposer, { item: baseItem, runtime: baseRuntime });
    const textarea = screen.getByPlaceholderText('Nhập tin nhắn...');

    await fireEvent.paste(textarea);
    await waitFor(() => expect(mockStageClipboard).toHaveBeenCalledTimes(1));

    dragActiveHandler?.(true);
    await tick();
    expect(screen.getByTestId('attachment-drop-overlay')).toBeTruthy();
    dragDropHandler?.(['/tmp/a.png']);
    await waitFor(() => expect(mockStagePaths).toHaveBeenCalled());
  });
});
