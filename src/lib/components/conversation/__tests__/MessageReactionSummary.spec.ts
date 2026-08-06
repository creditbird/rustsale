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
    expect(onToggleReaction).toHaveBeenCalledWith('heart', true);

    const likeChip = screen.getByRole('button', { name: /👍 3/ });
    await fireEvent.click(likeChip);
    expect(onToggleReaction).toHaveBeenCalledWith('like', false);
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
    expect(onToggleReaction).toHaveBeenCalledWith('heart', true);
    expect(screen.queryByText(/\/-heart/)).toBeNull();
  });
});
