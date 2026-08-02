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
  providerMessageId: null,
  providerClientMessageId: `client-${id}`,
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

  it('renders one aggregate footer and a failed-part warning', () => {
    render(MessageBatchGroup, { messages: [makeMessage(1, 'sent'), makeMessage(2, 'failed')] });
    expect(screen.getAllByText('Đã gửi một phần')).toHaveLength(1);
    expect(screen.getByText('Network error')).toBeTruthy();
  });
});
