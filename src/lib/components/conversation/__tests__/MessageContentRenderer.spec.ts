import { afterEach, describe, it, expect } from 'bun:test';
import { cleanup, render } from '@testing-library/svelte';
import MessageContentRenderer from '../MessageContentRenderer.svelte';
import type { ConversationMessage } from '$lib/services/conversationService';

describe('MessageContentRenderer', () => {
  afterEach(cleanup);

  const baseMessage: ConversationMessage = {
    id: 101,
    conversationId: 31,
    channelAccountId: 4,
    channel: 'zalo',
    accountDisplayName: null,
    senderIdentityId: 1,
    senderDisplayName: 'Nguyen Van A',
    direction: 'inbound',
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
    batchIndex: null,
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

  it('renders video, audio, and safe link attachments with dedicated components', () => {
    const richMessage: ConversationMessage = {
      ...baseMessage,
      contentType: 'link',
      content: {
        url: 'https://example.com/article',
        title: 'Bài viết an toàn',
        description: 'Mô tả liên kết'
      },
      attachments: [
        {
          id: 2,
          mediaType: 'video',
          mimeType: 'video/mp4',
          fileName: 'clip.mp4',
          sizeBytes: 2048,
          remoteUrl: 'https://example.com/clip.mp4',
          thumbnailUrl: null,
          width: 640,
          height: 360,
          durationSeconds: 12
        },
        {
          id: 3,
          mediaType: 'audio',
          mimeType: 'audio/mpeg',
          fileName: 'voice.mp3',
          sizeBytes: 512,
          remoteUrl: 'https://example.com/voice.mp3',
          thumbnailUrl: null,
          width: null,
          height: null,
          durationSeconds: 8
        }
      ]
    };

    const { getByTestId, getByText } = render(MessageContentRenderer, { message: richMessage });
    expect(getByTestId('video-message-content')).toBeTruthy();
    expect(getByTestId('audio-message-content')).toBeTruthy();
    expect(getByTestId('link-message-content')).toBeTruthy();
    expect(getByText('Bài viết an toàn')).toBeTruthy();
  });

  it('does not render unsafe link protocols', () => {
    const unsafeLink: ConversationMessage = {
      ...baseMessage,
      contentType: 'link',
      textContent: null,
      content: { url: 'javascript:alert(1)', title: 'Unsafe' }
    };

    const { queryByRole, getByText } = render(MessageContentRenderer, { message: unsafeLink });
    expect(queryByRole('link')).toBeNull();
    expect(getByText('Liên kết không hợp lệ')).toBeTruthy();
  });

  it('renders sticker message with image when URL is present in content object or attachments', () => {
    const stickerMsgWithContentUrl: ConversationMessage = {
      ...baseMessage,
      contentType: 'text',
      textContent: '[Sticker]',
      content: { spriteUrl: 'https://cdn.example.com/sticker-sprite.png' },
      attachments: [],
    };

    const { getByTestId } = render(MessageContentRenderer, { message: stickerMsgWithContentUrl });
    expect(getByTestId('sticker-message-content')).toBeTruthy();
  });

  it('suppresses link preview card for video links and video attachments', () => {
    const zaloVideoMsg: ConversationMessage = {
      ...baseMessage,
      contentType: 'video',
      textContent: null,
      content: {
        href: 'https://video-stal-7.dlmd.me/no/b2cc1e68da491c174558/2aOboQq1lpmg2o0f39pSouiyKjfpjfkQdTHn1SvQ',
        title: 'https://video-stal-7.dlmd.me/no/b2cc1e68da491c174558/2aOboQq1lpmg2o0f39pSouiyKjfpjfkQdTHn1SvQ',
        description: 'https://video-stal-7.dlmd.me/no/b2cc1e68da491c174558/2aOboQq1lpmg2o0f39pSouiyKjfpjfkQdTHn1SvQ',
        thumbUrl: 'https://photo-stal-7.dlmd.me/thumb.jpg'
      },
      attachments: [
        {
          id: 99,
          mediaType: 'video',
          mimeType: 'video/mp4',
          fileName: 'video.mp4',
          sizeBytes: 5000,
          remoteUrl: 'https://video-stal-7.dlmd.me/no/b2cc1e68da491c174558/2aOboQq1lpmg2o0f39pSouiyKjfpjfkQdTHn1SvQ',
          thumbnailUrl: 'https://photo-stal-7.dlmd.me/thumb.jpg',
          width: 720,
          height: 1280,
          durationSeconds: 15
        }
      ]
    };

    const { getByTestId, queryByTestId } = render(MessageContentRenderer, { message: zaloVideoMsg });
    expect(getByTestId('video-message-content')).toBeTruthy();
    expect(queryByTestId('link-message-content')).toBeNull();
    expect(queryByTestId('message-text-content')).toBeNull();
  });
});
