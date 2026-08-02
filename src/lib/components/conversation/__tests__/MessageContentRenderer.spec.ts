import { describe, it, expect } from 'bun:test';
import { render } from '@testing-library/svelte';
import MessageContentRenderer from '../MessageContentRenderer.svelte';
import type { ConversationMessage } from '$lib/services/conversationService';

describe('MessageContentRenderer', () => {
  const baseMessage: ConversationMessage = {
    id: 101,
    conversationId: 31,
    channelAccountId: 4,
    channel: 'zalo',
    accountDisplayName: null,
    senderIdentityId: 1,
    senderDisplayName: 'Nguyen Van A',
    direction: 'inbound',
    providerMessageId: 'p-101',
    contentType: 'text',
    textContent: 'Xin chào bạn',
    content: {},
    occurredAt: '2026-08-02T10:00:00.000Z',
    status: 'sent',
    removedAt: null,
    errorMessage: null,
    replyToMessageId: null,
    attachments: [],
    replyTarget: null,
    reactions: [],
    sendBatch: null,
  };

  it('renders plain text content', () => {
    const { getByTestId, getByText } = render(MessageContentRenderer, { message: baseMessage });
    expect(getByTestId('message-content-renderer')).not.toBeNull();
    expect(getByText('Xin chào bạn')).not.toBeNull();
  });

  it('renders image attachment', () => {
    const msgWithImage: ConversationMessage = {
      ...baseMessage,
      attachments: [
        {
          id: 1,
          mediaType: 'image',
          mimeType: 'image/png',
          fileName: 'photo.png',
          sizeBytes: 1024,
          localPath: null,
          remoteUrl: 'https://example.com/photo.png',
          thumbnailUrl: null,
          width: 800,
          height: 600,
          durationSeconds: null,
        },
      ],
    };

    const { getByTestId } = render(MessageContentRenderer, { message: msgWithImage });
    expect(getByTestId('image-message-content')).not.toBeNull();
  });
});
