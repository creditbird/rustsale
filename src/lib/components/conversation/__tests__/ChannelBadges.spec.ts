/// <reference types="bun-types" />
import { afterEach, describe, expect, test, mock } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import type { Channel, ChannelBadgeSummary } from '$lib/services/conversationService';
import ChannelBadges from '../ChannelBadges.svelte';

describe('ChannelBadges Component', () => {
  const mockChannels: ChannelBadgeSummary[] = [
    { channel: 'zalo', messageCount: 15 },
    { channel: 'messenger', messageCount: 3 }
  ];

  afterEach(() => {
    cleanup();
  });

  test('renders only available channels with icons and counts', () => {
    render(ChannelBadges, {
      props: {
        channels: mockChannels,
        activeChannels: ['zalo', 'messenger'],
        onToggle: () => {}
      }
    });

    expect(screen.getByText('15')).toBeTruthy();
    expect(screen.getByText('3')).toBeTruthy();
    expect(screen.getByRole('button', { name: /zalo/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /messenger/i })).toBeTruthy();
    expect(screen.queryByRole('button', { name: /whatsapp/i })).toBeNull();
    expect(screen.queryByRole('button', { name: /email/i })).toBeNull();
  });

  test('aria-pressed reflects active state', () => {
    render(ChannelBadges, {
      props: {
        channels: mockChannels,
        activeChannels: ['zalo'],
        onToggle: () => {}
      }
    });

    const zaloBtn = screen.getByRole('button', { name: /zalo/i });
    const messengerBtn = screen.getByRole('button', { name: /messenger/i });

    expect(zaloBtn.getAttribute('aria-pressed')).toBe('true');
    expect(messengerBtn.getAttribute('aria-pressed')).toBe('false');
  });

  test('clicking a badge invokes onToggle with channel name', async () => {
    const onToggle = mock((ch: Channel) => {});
    render(ChannelBadges, {
      props: {
        channels: mockChannels,
        activeChannels: ['zalo', 'messenger'],
        onToggle
      }
    });

    const messengerBtn = screen.getByRole('button', { name: /messenger/i });
    await fireEvent.click(messengerBtn);

    expect(onToggle).toHaveBeenCalledTimes(1);
    expect(onToggle).toHaveBeenCalledWith('messenger');
  });

  test('displays message when active channels set is empty', () => {
    render(ChannelBadges, {
      props: {
        channels: mockChannels,
        activeChannels: [],
        onToggle: () => {}
      }
    });

    expect(screen.getByText('Không có kênh nào đang được hiển thị')).toBeTruthy();
  });
});
