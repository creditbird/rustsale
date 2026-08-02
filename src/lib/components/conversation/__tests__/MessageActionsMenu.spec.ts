import { afterEach, describe, expect, it, mock } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import MessageActionsMenu from '../MessageActionsMenu.svelte';

describe('MessageActionsMenu', () => {
  afterEach(cleanup);

  it('uses the same eligible actions for the explicit menu', async () => {
    const onReply = mock(() => {});
    const onCopy = mock(() => {});
    const onRecall = mock(() => {});
    render(MessageActionsMenu, {
      canReply: true,
      canCopy: true,
      canRecall: false,
      onReply,
      onCopy,
      onRecall
    });

    await fireEvent.click(screen.getByTestId('message-actions-trigger'));
    await fireEvent.click(await screen.findByText('Trả lời'));
    expect(onReply).toHaveBeenCalledTimes(1);
    expect(screen.queryByText('Thu hồi')).toBeNull();
  });
});
