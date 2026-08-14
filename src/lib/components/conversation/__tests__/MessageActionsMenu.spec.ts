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
    await fireEvent.click(await screen.findByText('Reply'));
    expect(onReply).toHaveBeenCalledTimes(1);
    expect(screen.queryByText('Recall')).toBeNull();
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
    const heartQuickBtn = screen.getByRole('button', { name: /React ❤️/i });
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
    const forwardBtn = await screen.findByText('Forward');
    expect(forwardBtn).toBeTruthy();

    await fireEvent.click(forwardBtn);
    expect(onForward).toHaveBeenCalledTimes(1);
  });

  it('closes when clicking outside the menu', async () => {
    render(MessageActionsMenu, {
      canReply: true,
      canCopy: true,
      canRecall: false,
      onReply: () => {},
      onCopy: () => {},
      onRecall: () => {}
    });

    await fireEvent.click(screen.getByTestId('message-actions-trigger'));
    expect(screen.getByText('Reply')).toBeTruthy();

    await new Promise((resolve) => setTimeout(resolve, 20));
    document.body.dispatchEvent(
      new PointerEvent('pointerdown', {
        bubbles: true,
        pointerType: 'mouse',
        clientX: 100,
        clientY: 100
      })
    );
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(screen.getByTestId('message-actions-trigger').getAttribute('aria-expanded')).toBe('false');
  });
});
