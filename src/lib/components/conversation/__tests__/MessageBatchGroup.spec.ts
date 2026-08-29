import { afterEach, describe, expect, it } from 'bun:test';
import { cleanup, render, screen } from '@testing-library/svelte';
import type { ConversationMessage } from '$lib/services/conversationService';
import MessageBatchGroup from '../MessageBatchGroup.svelte';

const makeMessage = (id: number, status: ConversationMessage['status']): ConversationMessage => ({
  id,
  conversationId: 31,
  channelAccountId: 4,
  channel: 'zalo',
  accountDisplayName: 'Zalo Test',
  senderIdentityId: null,
  senderDisplayName: null,
  direction: 'outbound',
  contentType: 'text',
  textContent: `Part ${id}`,
  content: {},
  occurredAt: `2026-08-02T10:00:0${id}.000Z`,
  status,
  removedAt: null,
  errorMessage: status === 'failed' ? 'Network error' : null,
  replyToMessageId: null,
  attachments: [],
  replyTarget: null,
  reactions: [],
  batchIndex: id - 1,
  sendBatch: {
    sendBatchId: 'batch-a',
    status: status === 'failed' ? 'partial' : 'sending',
    totalParts: 2,
    sentParts: 1,
    failedParts: status === 'failed' ? 1 : 0
  }
});

describe('MessageBatchGroup', () => {
  afterEach(cleanup);

  it('renders "Sent" when all messages are sent successfully', () => {
    render(MessageBatchGroup, { messages: [makeMessage(1, 'sent')] });
    expect(screen.getAllByText('Sent')).toHaveLength(1);
  });

  it('renders "Delivered" when all messages are delivered', () => {
    render(MessageBatchGroup, { messages: [makeMessage(1, 'delivered'), makeMessage(2, 'delivered')] });
    expect(screen.getAllByText('Delivered')).toHaveLength(1);
  });

  it('renders "Read" with CheckCheck icon and blue color when all messages are read', () => {
    const { container } = render(MessageBatchGroup, { messages: [makeMessage(1, 'read'), makeMessage(2, 'read')] });
    expect(screen.getAllByText('Read')).toHaveLength(1);
    const readIcon = container.querySelector('svg.lucide-check-check.text-\\[\\#53bdeb\\]');
    expect(readIcon).toBeTruthy();
    expect(readIcon?.classList.contains('lucide-check-check')).toBe(true);
  });

  it('renders one aggregate footer and a failed-part warning for partial batches', () => {
    render(MessageBatchGroup, { messages: [makeMessage(1, 'sent'), makeMessage(2, 'failed')] });
    expect(screen.getAllByText('Partially sent')).toHaveLength(1);
    expect(screen.getByText('Network error')).toBeTruthy();
  });

  it('renders "Failed to send" when all messages in batch failed', () => {
    render(MessageBatchGroup, { messages: [makeMessage(1, 'failed'), makeMessage(2, 'failed')] });
    expect(screen.getAllByText('Failed to send')).toHaveLength(1);
  });
});
