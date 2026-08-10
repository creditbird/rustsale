import { afterEach, describe, expect, it, mock } from 'bun:test';
import { cleanup, render, screen } from '@testing-library/svelte';
import type { ConversationReactionMutationResult } from '$lib/services/conversationService';
import MessageReactionMutationState from '../MessageReactionMutationState.svelte';

const mockSetConversationReaction = mock(async (req: any): Promise<ConversationReactionMutationResult> => ({
  clientMutationId: req.clientMutationId,
  messageId: req.messageId,
  reaction: req.reaction,
  requestedActive: req.active,
  status: 'succeeded' as const,
  reactions: [
    {
      reactionIcon: '❤️',
      count: 1,
      includesMe: true,
      actors: [{ identityId: 1, externalId: 'self', displayName: 'Self', avatarUrl: null }]
    }
  ],
  errorCode: null,
  errorMessage: null
}));

mock.module('$lib/services/conversationService', () => ({
  setConversationReaction: (req: any) => mockSetConversationReaction(req)
}));

describe('MessageReactionMutationState Component', () => {
  afterEach(() => {
    cleanup();
    mockSetConversationReaction.mockClear();
  });

  it('manages pending mutations, locks duplicates, and invokes setConversationReaction', async () => {
    const onReactionsUpdated = mock(() => {});
    const { component } = render(MessageReactionMutationState, { onReactionsUpdated });

    expect(component.isPending(101, 'heart')).toBe(false);

    const promise1 = component.executeMutation(101, 'heart', 'remove');
    expect(component.isPending(101, 'heart')).toBe(true);

    // Duplicate call while pending should return null
    const duplicatePromise = component.executeMutation(101, 'heart', 'remove');
    expect(await duplicatePromise).toBeNull();

    const res1 = await promise1;
    expect(res1?.status).toBe('succeeded');
    expect(component.isPending(101, 'heart')).toBe(false);
    expect(onReactionsUpdated).toHaveBeenCalledWith(101, res1?.reactions);
  });

  it('sends the requested active state without toggling it', async () => {
    const { component } = render(MessageReactionMutationState, {});

    await component.executeMutation(101, 'heart', 'increment');

    expect(mockSetConversationReaction).toHaveBeenCalledTimes(1);
    expect(mockSetConversationReaction.mock.calls[0][0].active).toBe(true);
  });

  it('debounces increments into one delta request and resolves every caller', async () => {
    const { component } = render(MessageReactionMutationState, {});

    const promises = [
      component.executeMutation(101, 'heart', 'increment', 99, 20),
      component.executeMutation(101, 'heart', 'increment', 99, 20),
      component.executeMutation(101, 'heart', 'increment', 99, 20)
    ];

    await new Promise((resolve) => setTimeout(resolve, 50));

    expect(mockSetConversationReaction).toHaveBeenCalledTimes(1);
    expect(mockSetConversationReaction.mock.calls[0][0]).toMatchObject({
      messageId: 101,
      reaction: 'heart',
      active: true,
      count: 3
    });

    const allSettled = await Promise.race([
      Promise.all(promises).then(() => true),
      new Promise<boolean>((resolve) => setTimeout(() => resolve(false), 50))
    ]);
    expect(allSettled).toBe(true);
  });

  it('renders error notice when mutation fails', async () => {
    mockSetConversationReaction.mockImplementationOnce(async (req: any): Promise<ConversationReactionMutationResult> => ({
      clientMutationId: req.clientMutationId,
      messageId: req.messageId,
      reaction: req.reaction,
      requestedActive: req.active,
      status: 'failed' as const,
      reactions: [],
      errorCode: 'PROVIDER_ERROR',
      errorMessage: 'Không thể gửi thả cảm xúc tới Zalo'
    }));

    const { component } = render(MessageReactionMutationState, {});

    await component.executeMutation(101, 'angry', 'remove');

    expect(screen.getByTestId('reaction-mutation-errors')).toBeTruthy();
    expect(screen.getByText('Không thể gửi thả cảm xúc tới Zalo')).toBeTruthy();
  });

  it('renders the message from a typed Tauri error object', async () => {
    mockSetConversationReaction.mockImplementationOnce(async () => {
      throw {
        code: 'REACTION_NOT_SUPPORTED',
        message: 'Tin nhắn thiếu provider client message ID'
      };
    });

    const { component } = render(MessageReactionMutationState, {});
    await component.executeMutation(101, 'heart', 'remove');

    expect(screen.getByText('Tin nhắn thiếu provider client message ID')).toBeTruthy();
    expect(screen.queryByText('[object Object]')).toBeNull();
  });
});
