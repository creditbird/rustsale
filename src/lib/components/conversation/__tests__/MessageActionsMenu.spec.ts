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

  it('renders quick reaction options when canReact is true', async () => {
    const onToggleReaction = mock(() => {});
    render(MessageActionsMenu, {
      canReply: true,
      canCopy: true,
      canRecall: false,
      canReact: true,
      onReply: () => {},
      onCopy: () => {},
      onRecall: () => {},
      onToggleReaction
    });

    await fireEvent.click(screen.getByTestId('message-actions-trigger'));
    const heartQuickBtn = screen.getByRole('button', { name: /Thả cảm xúc ❤️/i });
    expect(heartQuickBtn).toBeTruthy();

    await fireEvent.click(heartQuickBtn);
    expect(onToggleReaction).toHaveBeenCalledWith('heart');
  });

  it('renders forward option when canForward is true and triggers onForward', async () => {
    const onForward = mock(() => {});
    render(MessageActionsMenu, {
      canReply: false,
      canCopy: false,
      canRecall: false,
      canForward: true,
      onReply: () => {},
      onCopy: () => {},
      onRecall: () => {},
      onForward
    });

    await fireEvent.click(screen.getByTestId('message-actions-trigger'));
    const forwardBtn = await screen.findByText('Chuyển tiếp');
    expect(forwardBtn).toBeTruthy();

    await fireEvent.click(forwardBtn);
    expect(onForward).toHaveBeenCalledTimes(1);
  });
});
