/// <reference types="bun-types" />
import { afterEach, describe, expect, mock, test } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import type {
  ConversationCapabilities,
  ConversationMessage,
  ConversationReactionMutationResult
} from '$lib/services/conversationService';
import MessageBubble from '../MessageBubble.svelte';
import * as m from '$lib/paraglide/messages';

describe('MessageBubble Component', () => {
  afterEach(() => {
    cleanup();
  });

  const baseMessage: ConversationMessage = {
    id: 101,
    conversationId: 1,
    channelAccountId: 10,
    channel: 'zalo',
    accountDisplayName: 'Zalo Official Account',
    senderIdentityId: 201,
    senderDisplayName: 'Trần Văn B',
    direction: 'inbound',
    contentType: 'text',
    textContent: 'Xin chào shop, em muốn tư vấn sản phẩm',
    content: {},
    occurredAt: '2026-07-31T14:30:00Z',
    status: 'received',
    removedAt: null,
    errorMessage: null,
    replyToMessageId: null,
    attachments: [],
    replyTarget: null,
    reactions: [],
    batchIndex: null,
    sendBatch: null,
    actions: { reply: true, copy: true, recall: true, forward: true, react: true }
  };

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

  test('inbound text shows sender, channel, account, and Vietnamese-localized timestamp', () => {
    render(MessageBubble, {
      props: {
        message: baseMessage,
        isGroup: false
      }
    });

    expect(screen.getByText('Trần Văn B')).toBeTruthy();
    expect(screen.getAllByText(/zalo/i).length).toBeGreaterThan(0);
    expect(screen.getByText('Zalo Official Account')).toBeTruthy();
    expect(screen.getByText('Xin chào shop, em muốn tư vấn sản phẩm')).toBeTruthy();

    const formattedTime = new Intl.DateTimeFormat('vi-VN', {
      dateStyle: 'short',
      timeStyle: 'short'
    }).format(new Date('2026-07-31T14:30:00Z'));

    // Verify day/month/year/time numbers appear in formatted string
    expect(screen.getByText(new RegExp('14:30'))).toBeTruthy();
  });

  test('outbound status labels (queued, sending, sent, delivered, read, failed)', () => {
    const statuses: Array<{ status: ConversationMessage['status']; label: RegExp }> = [
      { status: 'queued', label: /Đang chờ|queued/i },
      { status: 'sending', label: /Đang gửi|sending/i },
      { status: 'sent', label: /Đã gửi|sent/i },
      { status: 'delivered', label: /Đã phát|Đã nhận|delivered/i },
      { status: 'read', label: /Đã xem|read/i },
      { status: 'failed', label: /Thất bại|failed/i }
    ];

    for (const { status, label } of statuses) {
      const outboundMessage: ConversationMessage = {
        ...baseMessage,
        id: 102,
        direction: 'outbound',
        senderDisplayName: null,
        status
      };

      const { unmount } = render(MessageBubble, {
        props: {
          message: outboundMessage,
          isGroup: false
        }
      });

      expect(screen.getByText(label)).toBeTruthy();
      unmount();
    }
  });

  test('removed message renders a tombstone and hides original text', () => {
    const removedMessage: ConversationMessage = {
      ...baseMessage,
      status: 'removed',
      removedAt: '2026-07-31T15:00:00Z',
      textContent: 'Nội dung bí mật đã bị xóa'
      ,reactions: [
        {
          reactionIcon: '❤️',
          count: 1,
          includesMe: true,
          actors: [
            { identityId: null, externalId: 'own-zuid', displayName: null, avatarUrl: null }
          ]
        }
      ]
    };

    render(MessageBubble, {
      props: {
        message: removedMessage,
        isGroup: false
      }
    });

    expect(screen.getByText(m.conversation_message_recalled())).toBeTruthy();
    expect(screen.queryByText('Nội dung bí mật đã bị xóa')).toBeNull();
    expect(screen.queryByTestId('message-reaction-summary')).toBeNull();
  });

  test('reply and recall eligibility are backend-action driven', async () => {
    const onReply = mock(() => {});
    const onRecall = mock(() => {});
    const outbound: ConversationMessage = {
      ...baseMessage,
      direction: 'outbound',
      status: 'sent',
      occurredAt: new Date().toISOString()
    };
    render(MessageBubble, {
      message: outbound,
      capabilities: richCapabilities,
      onReply,
      onRecall
    });

    await fireEvent.click(screen.getByTestId('message-actions-trigger'));
    expect(screen.getByText(m.conversation_reply())).toBeTruthy();
    expect(screen.getByText(m.conversation_recall())).toBeTruthy();
    await fireEvent.click(screen.getByText(m.conversation_recall()));
    expect(onRecall).toHaveBeenCalledWith(outbound);
  });

  test('write-disabled reactions remain visible but expose no mutation action', async () => {
    const message: ConversationMessage = {
      ...baseMessage,
      reactions: [
        {
          reactionIcon: '❤️',
          count: 1,
          includesMe: false,
          actors: []
        }
      ],
      actions: undefined
    };
    const capabilities: ConversationCapabilities = {
      ...richCapabilities,
      reactions: {
        read: true,
        write: false,
        supported: ['heart']
      }
    };

    render(MessageBubble, { message, capabilities });

    expect(screen.getByRole('button', { name: /❤️ 1/ })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Add reaction' })).toBeNull();
    await fireEvent.click(screen.getByTestId('message-actions-trigger'));
    expect(screen.queryByText(/React with ❤️/)).toBeNull();
  });

  test('right-click and Shift+F10 open the same explicit actions', async () => {
    const onReply = mock(() => {});
    const { container } = render(MessageBubble, {
      message: baseMessage,
      capabilities: richCapabilities,
      onReply
    });
    const bubble = container.querySelector('[data-message-id="101"]') as HTMLElement;

    await fireEvent.contextMenu(bubble);
    await fireEvent.click(screen.getByText(m.conversation_reply()));
    expect(onReply).toHaveBeenCalledTimes(1);

    await fireEvent.keyDown(screen.getByTestId('message-actions-trigger'), {
      key: 'F10',
      shiftKey: true
    });
    await fireEvent.click(screen.getByText(m.conversation_reply()));
    expect(onReply).toHaveBeenCalledTimes(2);
  });

  test('reaction picker closes when clicking outside', async () => {
    render(MessageBubble, {
      message: baseMessage,
      capabilities: richCapabilities
    });

    await fireEvent.click(screen.getByRole('button', { name: 'Add reaction' }));
    expect(screen.getByRole('toolbar', { name: /Choose reaction/i })).toBeTruthy();

    await new Promise((resolve) => setTimeout(resolve, 20));
    await fireEvent.pointerDown(document.body, {
      pointerType: 'mouse',
      clientX: 100,
      clientY: 100
    });
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(screen.queryByRole('toolbar', { name: /Choose reaction/i })).toBeNull();
  });

  test('first reaction renders an optimistic chip before the mutation settles', async () => {
    let resolveMutation: ((result: ConversationReactionMutationResult | null) => void) | undefined;
    const mutation = new Promise<ConversationReactionMutationResult | null>((resolve) => {
      resolveMutation = resolve;
    });
    const onToggleReaction = mock(() => mutation);

    render(MessageBubble, {
      message: baseMessage,
      capabilities: richCapabilities,
      onToggleReaction
    });

    await fireEvent.click(screen.getByTestId('message-actions-trigger'));
    await fireEvent.click(screen.getByRole('button', { name: /React ❤️/i }));

    expect(screen.getByRole('button', { name: /❤️ 1/ })).toBeTruthy();
    expect(onToggleReaction).toHaveBeenCalledWith(baseMessage, 'heart', 'increment');

    resolveMutation?.(null);
    await mutation;
    await Promise.resolve();

    expect(screen.queryByRole('button', { name: /❤️ 1/ })).toBeNull();
  });

  test('contentType = email renders a card using content.subject, content.from, content.to without {@html}', () => {
    const emailMessage: ConversationMessage = {
      ...baseMessage,
      channel: 'email',
      contentType: 'email',
      textContent: 'Nội dung email dạng plain text',
      content: {
        subject: 'Báo giá sản phẩm 2026',
        from: 'doi-tac@example.com',
        to: ['sales@rustsale.vn']
      }
    };

    const { container } = render(MessageBubble, {
      props: {
        message: emailMessage,
        isGroup: false
      }
    });

    expect(screen.getByText(/Báo giá sản phẩm 2026/)).toBeTruthy();
    expect(screen.getByText(/doi-tac@example.com/)).toBeTruthy();
    expect(screen.getByText(/sales@rustsale.vn/)).toBeTruthy();
    expect(screen.getByText('Nội dung email dạng plain text')).toBeTruthy();

    // Ensure raw html isn't rendered via dangerous innerHTML/{@html}
    expect(container.innerHTML).not.toContain('<script>');
  });

  test('unsupported content type renders fallback and never injects HTML from content', () => {
    const unsupportedMessage: ConversationMessage = {
      ...baseMessage,
      contentType: 'unknown_custom_type',
      textContent: null,
      content: {
        unsafeData: '<img src=x onerror=alert(1)>'
      }
    };

    const { container } = render(MessageBubble, {
      props: {
        message: unsupportedMessage,
        isGroup: false
      }
    });

    expect(screen.getByText('Unsupported content')).toBeTruthy();
    expect(container.querySelector('img[onerror]')).toBeNull();
  });

  test('group sender name is visible while direct outbound messages do not invent a customer sender', () => {
    // Case 1: Inbound group message shows sender display name
    const groupInboundMessage: ConversationMessage = {
      ...baseMessage,
      senderDisplayName: 'Thành Viên Nhóm'
    };

    const { unmount: unmountGroup } = render(MessageBubble, {
      props: {
        message: groupInboundMessage,
        isGroup: true
      }
    });

    expect(screen.getByText('Thành Viên Nhóm')).toBeTruthy();
    unmountGroup();

    // Case 2: Outbound message does NOT render sender name if sender is customer or null
    const outboundDirectMessage: ConversationMessage = {
      ...baseMessage,
      direction: 'outbound',
      senderDisplayName: null,
      textContent: 'Tin nhắn gửi đi'
    };

    render(MessageBubble, {
      props: {
        message: outboundDirectMessage,
        isGroup: false
      }
    });

    expect(screen.queryByText('Trần Văn B')).toBeNull();
    expect(screen.getByText('Tin nhắn gửi đi')).toBeTruthy();
  });

  test('inbound message matching account display name does not display account name as sender', () => {
    const pollutedMessage: ConversationMessage = {
      ...baseMessage,
      accountDisplayName: 'Thanh Truyền',
      senderDisplayName: 'Thanh Truyền',
      textContent: 'Cuộc gọi'
    };

    const { container } = render(MessageBubble, {
      props: {
        message: pollutedMessage,
        isGroup: false
      }
    });

    expect(container.querySelector('[data-slot="sender-name"]')).toBeNull();
  });

  test('validates safe image media schemes and renders fallback card for unsafe schemes', () => {
    // Safe image URL
    const safeImageMsg: ConversationMessage = {
      ...baseMessage,
      contentType: 'image',
      content: {
        url: 'https://cdn.example.com/photo.png'
      }
    };

    const { unmount: unmountSafe } = render(MessageBubble, {
      props: {
        message: safeImageMsg,
        isGroup: false
      }
    });

    const img = screen.getByRole('img');
    expect(img.getAttribute('src')).toBe('https://cdn.example.com/photo.png');
    unmountSafe();

    // Unsafe image URL (javascript:)
    const unsafeImageMsg: ConversationMessage = {
      ...baseMessage,
      contentType: 'image',
      content: {
        url: 'javascript:alert(1)'
      }
    };

    render(MessageBubble, {
      props: {
        message: unsafeImageMsg,
        isGroup: false
      }
    });

    expect(screen.queryByRole('img')).toBeNull();
    expect(screen.getByText('Invalid attachment file')).toBeTruthy();
  });

  test('renders image caption or description from content object when textContent is null', () => {
    const imageWithCaptionMsg: ConversationMessage = {
      ...baseMessage,
      contentType: 'image',
      textContent: null,
      content: {
        url: 'https://cdn.example.com/photo.png',
        caption: 'Chào tháng 8 may mắn'
      }
    };

    render(MessageBubble, {
      props: {
        message: imageWithCaptionMsg,
        isGroup: false
      }
    });

    const img = screen.getByRole('img');
    expect(img.getAttribute('src')).toBe('https://cdn.example.com/photo.png');
    expect(screen.getByText('Chào tháng 8 may mắn')).toBeTruthy();
  });

  test('renders text message with mention highlights when mentions array is provided', () => {
    const mentionMessage: ConversationMessage = {
      ...baseMessage,
      textContent: 'Xin chào @Nguyễn Văn A trong nhóm',
      mentions: [
        {
          identityId: 501,
          externalId: 'ext-501',
          displayText: '@Nguyễn Văn A',
          startUtf16: 9,
          lengthUtf16: 13
        }
      ]
    };

    const { getAllByTestId } = render(MessageBubble, {
      props: {
        message: mentionMessage,
        isGroup: true
      }
    });

    const mentionSpans = getAllByTestId('mention-span');
    expect(mentionSpans).toHaveLength(1);
    expect(mentionSpans[0].textContent).toBe('@Nguyễn Văn A');
    expect(mentionSpans[0].getAttribute('data-mention-id')).toBe('501');
  });

  test('renders WhatsApp channel message with emerald badge, sender phone number, and delivery receipts', () => {
    const whatsappMsg: ConversationMessage = {
      ...baseMessage,
      id: 701,
      channel: 'whatsapp',
      accountDisplayName: 'WA Business (+84987654321)',
      senderDisplayName: '+84912345678',
      direction: 'inbound',
      textContent: 'Hello from WhatsApp user'
    };

    const { unmount } = render(MessageBubble, {
      props: {
        message: whatsappMsg,
        isGroup: false
      }
    });

    expect(screen.getByText('+84912345678')).toBeTruthy();
    expect(screen.getByText('whatsapp')).toBeTruthy();
    expect(screen.getByText('WA Business (+84987654321)')).toBeTruthy();
    expect(screen.getByText('Hello from WhatsApp user')).toBeTruthy();
    unmount();

    // Outbound WhatsApp message with read delivery receipt
    const outboundWa: ConversationMessage = {
      ...whatsappMsg,
      id: 702,
      direction: 'outbound',
      status: 'read'
    };

    render(MessageBubble, {
      props: {
        message: outboundWa,
        isGroup: false
      }
    });

    expect(screen.getByText('Read')).toBeTruthy();
  });

  test('renders WhatsApp quoted reply preview correctly', () => {
    const waReplyMsg: ConversationMessage = {
      ...baseMessage,
      id: 703,
      channel: 'whatsapp',
      textContent: 'I agree with this proposal',
      replyTarget: {
        messageId: 700,
        senderDisplayName: '+84912345678',
        contentType: 'text',
        textContent: 'Can you send the product catalog?',
        mediaType: null,
        occurredAt: '2026-08-02T10:00:00Z'
      }
    };

    render(MessageBubble, {
      props: {
        message: waReplyMsg,
        isGroup: false
      }
    });

    expect(screen.getByText('Can you send the product catalog?')).toBeTruthy();
    expect(screen.getByText('+84912345678')).toBeTruthy();
    expect(screen.getByText('I agree with this proposal')).toBeTruthy();
  });

  test('renders WhatsApp group message with real sender name and phone number on the right', () => {
    const groupMsg: ConversationMessage = {
      ...baseMessage,
      id: 704,
      channel: 'whatsapp',
      senderDisplayName: 'Sadia Wajiha',
      senderExternalId: '447503823686',
      senderAvatarUrl: null,
      textContent: 'Hi friends!'
    };

    render(MessageBubble, {
      props: {
        message: groupMsg,
        isGroup: true,
        conversationTitle: 'AI Builders Tech Q&A'
      }
    });

    expect(screen.getByText('Sadia Wajiha')).toBeTruthy();
    expect(screen.queryByText(/~/)).toBeNull();
    expect(screen.getByText('+44 7503 823686')).toBeTruthy();
    expect(screen.getByText('SW')).toBeTruthy();
  });

  test('ignores group conversation title as sender name and falls back to phone number', () => {
    const groupMsg: ConversationMessage = {
      ...baseMessage,
      id: 705,
      channel: 'whatsapp',
      senderDisplayName: 'AI Builders Tech Q&A',
      senderExternalId: '84978781822',
      senderAvatarUrl: null,
      textContent: 'Hi builders'
    };

    render(MessageBubble, {
      props: {
        message: groupMsg,
        isGroup: true,
        conversationTitle: 'AI Builders Tech Q&A'
      }
    });

    expect(screen.queryByText(/AI Builders Tech Q&A/)).toBeNull();
    expect(screen.queryByText(/~/)).toBeNull();
    expect(screen.getByText(/\+84 97 878/)).toBeTruthy();
    expect(screen.getByText('#')).toBeTruthy();
  });

  test('renders Member without lone tilde when sender has no name and unmapped LID', () => {
    const groupMsg: ConversationMessage = {
      ...baseMessage,
      id: 706,
      channel: 'whatsapp',
      senderDisplayName: 'AI Builders Community',
      senderExternalId: '120363422595125899',
      senderAvatarUrl: null,
      textContent: 'Nice one'
    };

    render(MessageBubble, {
      props: {
        message: groupMsg,
        isGroup: true,
        conversationTitle: 'AI Builders Community'
      }
    });

    // Must NOT render lone ~
    expect(screen.queryByText(/~/)).toBeNull();
    // Must render Member
    expect(screen.getByText('Member')).toBeTruthy();
    // Must render initials 'M'
    expect(screen.getByText('M')).toBeTruthy();
  });
});
