import { afterEach, describe, expect, it } from 'bun:test';
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
      ]
    });

    const chip = screen.getByRole('button', { name: /❤️ 2/ });
    expect(chip.getAttribute('data-current-account')).toBe('true');
    await fireEvent.click(chip);
    expect(screen.getByText('own-zuid')).toBeTruthy();
    expect(screen.getByText('Khách hàng')).toBeTruthy();
    expect(screen.queryByRole('button', { name: /thêm reaction/i })).toBeNull();
  });
});
