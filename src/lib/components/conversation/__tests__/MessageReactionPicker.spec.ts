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

    const picker = screen.getByRole('toolbar', { name: /Chọn cảm xúc/i });
    expect(picker).toBeTruthy();

    const buttons = screen.getAllByRole('button');
    expect(buttons.length).toBe(6);

    const heartBtn = screen.getByRole('button', { name: /Yêu thích/i });
    expect(heartBtn.getAttribute('aria-pressed')).toBe('true');

    const likeBtn = screen.getByRole('button', { name: /^Thích$/i });
    expect(likeBtn.getAttribute('aria-pressed')).toBe('false');

    expect(screen.getByRole('button', { name: /Cười/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /Bất ngờ/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /Buồn/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /Tức giận/i })).toBeTruthy();
  });

  it('handles clicking reaction icons and passes toggled state', async () => {
    const onSelect = mock(() => {});

    render(MessageReactionPicker, {
      messageId: 101,
      activeReactions: ['heart'],
      onSelect
    });

    const heartBtn = screen.getByRole('button', { name: /Yêu thích/i });
    await fireEvent.click(heartBtn);
    expect(onSelect).toHaveBeenCalledWith('heart', true);

    const likeBtn = screen.getByRole('button', { name: /^Thích$/i });
    await fireEvent.click(likeBtn);
    expect(onSelect).toHaveBeenCalledWith('like', false);
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

    const picker = screen.getByRole('toolbar', { name: /Chọn cảm xúc/i });
    const buttons = screen.getAllByRole('button');

    buttons[0].focus();
    expect(document.activeElement).toBe(buttons[0]);

    await fireEvent.keyDown(picker, { key: 'ArrowRight' });
    expect(document.activeElement).toBe(buttons[1]);

    await fireEvent.keyDown(picker, { key: 'ArrowLeft' });
    expect(document.activeElement).toBe(buttons[0]);

    await fireEvent.keyDown(buttons[0], { key: 'Enter' });
    expect(onSelect).toHaveBeenCalledWith('heart', false);

    await fireEvent.keyDown(picker, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('disables buttons when in disabledReactions list', () => {
    render(MessageReactionPicker, {
      messageId: 101,
      disabledReactions: ['like']
    });

    const likeBtn = screen.getByRole('button', { name: /^Thích$/i });
    expect(likeBtn.hasAttribute('disabled')).toBe(true);
  });

  it('renders only reactions allowed by capability', () => {
    render(MessageReactionPicker, {
      messageId: 101,
      supportedReactions: ['heart', 'like']
    });

    expect(screen.getAllByRole('button')).toHaveLength(2);
    expect(screen.getByRole('button', { name: /Yêu thích/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /^Thích$/i })).toBeTruthy();
    expect(screen.queryByRole('button', { name: /Cười/i })).toBeNull();
  });
});
