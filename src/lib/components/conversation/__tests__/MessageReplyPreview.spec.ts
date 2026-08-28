/// <reference types="bun-types" />
import { afterEach, describe, expect, mock, test } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import type { ConversationReplyTarget } from '$lib/services/conversationService';
import MessageReplyPreview from '../MessageReplyPreview.svelte';

describe('MessageReplyPreview Component', () => {
  afterEach(() => {
    cleanup();
  });

  test('renders clean sender name without leading tildes', () => {
    const replyTarget: ConversationReplyTarget = {
      messageId: 101,
      senderDisplayName: '~Minh',
      contentType: 'text',
      textContent: 'Quoted message content',
      mediaType: null,
      occurredAt: '2026-08-26T12:00:00Z'
    };

    render(MessageReplyPreview, { props: { replyTarget } });

    expect(screen.getByText('Minh')).toBeTruthy();
    expect(screen.queryByText(/~Minh/)).toBeNull();
    expect(screen.getByText('Quoted message content')).toBeTruthy();
  });

  test('renders normal sender name without modification', () => {
    const replyTarget: ConversationReplyTarget = {
      messageId: 102,
      senderDisplayName: 'Sadia Wajiha',
      contentType: 'text',
      textContent: 'Hello world',
      mediaType: null,
      occurredAt: '2026-08-26T12:00:00Z'
    };

    render(MessageReplyPreview, { props: { replyTarget } });

    expect(screen.getByText('Sadia Wajiha')).toBeTruthy();
  });

  test('renders Message when senderDisplayName is missing or empty', () => {
    const replyTarget: ConversationReplyTarget = {
      messageId: 103,
      senderDisplayName: null,
      contentType: 'text',
      textContent: 'Anonymous quoted text',
      mediaType: null,
      occurredAt: '2026-08-26T12:00:00Z'
    };

    render(MessageReplyPreview, { props: { replyTarget } });

    expect(screen.getByText('Message')).toBeTruthy();
  });

  test('fires onClick callback when clicked', async () => {
    const handleClick = mock(() => {});
    const replyTarget: ConversationReplyTarget = {
      messageId: 104,
      senderDisplayName: 'Dean H',
      contentType: 'text',
      textContent: 'Click me to jump',
      mediaType: null,
      occurredAt: '2026-08-26T12:00:00Z'
    };

    render(MessageReplyPreview, { props: { replyTarget, onClick: handleClick } });

    const preview = screen.getByTestId('message-reply-preview');
    await fireEvent.click(preview);

    expect(handleClick).toHaveBeenCalledTimes(1);
  });
});
