import { afterEach, describe, expect, it, mock } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import MessageReactionSummary from '../MessageReactionSummary.svelte';

describe('MessageReactionSummary', () => {
  afterEach(cleanup);

  it('renders read-only current-account chips and actor fallback names', async () => {
    render(MessageReactionSummary, {
      groups: [
        {
          reactionIcon: '❤️',
          count: 2,
          includesMe: true,
          actors: [
            { identityId: null, externalId: 'own-zuid', displayName: null, avatarUrl: null },
            { identityId: 7, externalId: 'peer-zuid', displayName: 'Khách hàng', avatarUrl: null }
          ]
        }
      ],
      canReact: false
    });

    const chip = screen.getByRole('button', { name: /❤️ 2/ });
    expect(chip.getAttribute('data-current-account')).toBe('true');
    await fireEvent.click(chip);
    expect(screen.getByText('own-zuid')).toBeTruthy();
    expect(screen.getByText('Khách hàng')).toBeTruthy();
    expect(screen.queryByRole('button', { name: /thêm reaction/i })).toBeNull();
  });

  it('triggers onToggleReaction when reaction chip is clicked and write capability is enabled', async () => {
    const onToggleReaction = mock(() => {});

    render(MessageReactionSummary, {
      groups: [
        {
          reactionIcon: '❤️',
          count: 1,
          includesMe: true,
          actors: [
            { identityId: null, externalId: 'own-zuid', displayName: 'Self', avatarUrl: null }
          ]
        },
        {
          reactionIcon: '👍',
          count: 3,
          includesMe: false,
          actors: [
            { identityId: 1, externalId: 'peer-1', displayName: 'Peer 1', avatarUrl: null }
          ]
        }
      ],
      canReact: true,
      onToggleReaction
    });

    const heartChip = screen.getByRole('button', { name: /❤️ 1/ });
    await fireEvent.click(heartChip);
    expect(onToggleReaction).toHaveBeenCalledWith('heart', 'increment');

    const likeChip = screen.getByRole('button', { name: /👍 3/ });
    await fireEvent.click(likeChip);
    expect(onToggleReaction).toHaveBeenCalledWith('like', 'increment');
  });

  it('renders provider aliases as emoji and toggles the canonical reaction', async () => {
    const onToggleReaction = mock(() => {});

    render(MessageReactionSummary, {
      groups: [
        {
          reactionIcon: '/-heart',
          count: 1,
          includesMe: true,
          actors: []
        }
      ],
      canReact: true,
      onToggleReaction
    });

    const chip = screen.getByRole('button', { name: /❤️ 1/ });
    await fireEvent.click(chip);
    expect(onToggleReaction).toHaveBeenCalledWith('heart', 'increment');
    expect(screen.queryByText(/\/-heart/)).toBeNull();
  });

  it('renders slash-colon provider aliases as canonical writable reactions', async () => {
    const onToggleReaction = mock(() => {});

    render(MessageReactionSummary, {
      groups: [
        {
          reactionIcon: '/:haha',
          count: 5,
          includesMe: false,
          actors: []
        },
        {
          reactionIcon: '/:sad',
          count: 2,
          includesMe: false,
          actors: []
        }
      ],
      canReact: true,
      onToggleReaction
    });

    const chip = screen.getByRole('button', { name: /😆 5/ });
    await fireEvent.click(chip);

    expect(onToggleReaction).toHaveBeenCalledWith('haha', 'increment');
    const sadChip = screen.getByRole('button', { name: /😭 2/ });
    await fireEvent.click(sadChip);
    expect(onToggleReaction).toHaveBeenCalledWith('cry', 'increment');
  });

  it('updates the displayed count optimistically until the mutation settles', async () => {
    let resolveMutation: (() => void) | undefined;
    const mutation = new Promise<void>((resolve) => {
      resolveMutation = resolve;
    });
    const onToggleReaction = mock(() => mutation);

    render(MessageReactionSummary, {
      groups: [
        {
          reactionIcon: '❤️',
          count: 2,
          includesMe: false,
          actors: []
        }
      ],
      canReact: true,
      onToggleReaction
    });

    await fireEvent.click(screen.getByRole('button', { name: /❤️ 2/ }));
    await fireEvent.click(screen.getByRole('button', { name: /❤️ 3/ }));

    expect(screen.getByRole('button', { name: /❤️ 4/ })).toBeTruthy();

    resolveMutation?.();
    await mutation;
    await Promise.resolve();

    expect(screen.getByRole('button', { name: /❤️ 2/ })).toBeTruthy();
  });

  it('removes only the current account reaction on context menu', async () => {
    const onToggleReaction = mock(() => {});

    render(MessageReactionSummary, {
      groups: [
        {
          reactionIcon: '❤️',
          count: 4,
          includesMe: true,
          actors: []
        },
        {
          reactionIcon: '👍',
          count: 3,
          includesMe: false,
          actors: []
        }
      ],
      canReact: true,
      onToggleReaction
    });

    await fireEvent.contextMenu(screen.getByRole('button', { name: /❤️ 4/ }));
    expect(onToggleReaction).toHaveBeenCalledWith('heart', 'remove');

    onToggleReaction.mockClear();
    await fireEvent.contextMenu(screen.getByRole('button', { name: /👍 3/ }));
    expect(onToggleReaction).not.toHaveBeenCalled();
  });

  it('renders unsupported provider icons read-only without defaulting to Like', async () => {
    const onToggleReaction = mock(() => {});

    render(MessageReactionSummary, {
      groups: [
        {
          reactionIcon: '/:new-haha',
          count: 5,
          includesMe: false,
          actors: [
            { identityId: null, externalId: 'actor-1', displayName: 'Người thả emoji', avatarUrl: null }
          ]
        }
      ],
      canReact: true,
      onToggleReaction
    });

    const chip = screen.getByRole('button', { name: /\/:new-haha 5/ });
    await fireEvent.click(chip);

    expect(onToggleReaction).not.toHaveBeenCalled();
    expect(screen.getByText('Người thả emoji')).toBeTruthy();
  });

  it('renders custom unicode emoji and toggles them as writable reaction', async () => {
    const onToggleReaction = mock(() => {});

    render(MessageReactionSummary, {
      groups: [
        {
          reactionIcon: '🙏',
          count: 1,
          includesMe: true,
          actors: [
            { identityId: null, externalId: 'own-uid', displayName: 'Me', avatarUrl: null }
          ]
        }
      ],
      canReact: true,
      onToggleReaction
    });

    const chip = screen.getByRole('button', { name: /🙏 1/ });
    await fireEvent.click(chip);
    expect(onToggleReaction).toHaveBeenCalledWith('🙏', 'increment');

    onToggleReaction.mockClear();
    await fireEvent.contextMenu(chip);
    expect(onToggleReaction).toHaveBeenCalledWith('🙏', 'remove');
  });
});
