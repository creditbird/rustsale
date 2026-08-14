import { afterEach, describe, expect, it, mock } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import MessageReactionPicker from '../MessageReactionPicker.svelte';

describe('MessageReactionPicker Component', () => {
  afterEach(cleanup);

  it('renders all 6 reaction icons with screen-reader labels and active markers', () => {
    const onSelect = mock(() => {});
    const onClose = mock(() => {});

    render(MessageReactionPicker, {
      messageId: 101,
      activeReactions: ['heart'],
      onSelect,
      onClose
    });

    const picker = screen.getByRole('toolbar', { name: /Choose reaction/i });
    expect(picker).toBeTruthy();

    const buttons = screen.getAllByRole('button');
    expect(buttons.length).toBe(6);

    const heartBtn = screen.getByRole('button', { name: /Love/i });
    expect(heartBtn.getAttribute('aria-pressed')).toBe('true');

    const likeBtn = screen.getByRole('button', { name: /^Like$/i });
    expect(likeBtn.getAttribute('aria-pressed')).toBe('false');

    expect(screen.getByRole('button', { name: /Haha/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /Wow/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /Sad/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /Angry/i })).toBeTruthy();
  });

  it('handles clicking reaction icons as explicit increments', async () => {
    const onSelect = mock(() => {});

    render(MessageReactionPicker, {
      messageId: 101,
      activeReactions: ['heart'],
      onSelect
    });

    const heartBtn = screen.getByRole('button', { name: /Love/i });
    await fireEvent.click(heartBtn);
    expect(onSelect).toHaveBeenCalledWith('heart', 'increment');

    const likeBtn = screen.getByRole('button', { name: /^Like$/i });
    await fireEvent.click(likeBtn);
    expect(onSelect).toHaveBeenCalledWith('like', 'increment');
  });

  it('supports keyboard navigation (ArrowLeft, ArrowRight, Enter, Space, Escape)', async () => {
    const onSelect = mock(() => {});
    const onClose = mock(() => {});

    render(MessageReactionPicker, {
      messageId: 101,
      activeReactions: [],
      onSelect,
      onClose
    });

    const picker = screen.getByRole('toolbar', { name: /Choose reaction/i });
    const buttons = screen.getAllByRole('button');

    buttons[0].focus();
    expect(document.activeElement).toBe(buttons[0]);

    await fireEvent.keyDown(picker, { key: 'ArrowRight' });
    expect(document.activeElement).toBe(buttons[1]);

    await fireEvent.keyDown(picker, { key: 'ArrowLeft' });
    expect(document.activeElement).toBe(buttons[0]);

    await fireEvent.keyDown(buttons[0], { key: 'Enter' });
    expect(onSelect).toHaveBeenCalledWith('heart', 'increment');

    await fireEvent.keyDown(picker, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('disables buttons when in disabledReactions list', () => {
    render(MessageReactionPicker, {
      messageId: 101,
      disabledReactions: ['like']
    });

    const likeBtn = screen.getByRole('button', { name: /^Like$/i });
    expect(likeBtn.hasAttribute('disabled')).toBe(true);
  });

  it('renders only reactions allowed by capability', () => {
    render(MessageReactionPicker, {
      messageId: 101,
      supportedReactions: ['heart', 'like']
    });

    expect(screen.getAllByRole('button')).toHaveLength(2);
    expect(screen.getByRole('button', { name: /Love/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /^Like$/i })).toBeTruthy();
    expect(screen.queryByRole('button', { name: /Haha/i })).toBeNull();
  });
});
