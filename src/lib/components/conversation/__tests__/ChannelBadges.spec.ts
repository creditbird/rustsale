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

  test('does not toggle off the final active channel', async () => {
    const onToggle = mock((ch: Channel) => {});
    render(ChannelBadges, {
      props: {
        channels: [{ channel: 'zalo', messageCount: 6 }],
        activeChannels: ['zalo'],
        onToggle
      }
    });

    const zaloBtn = screen.getByRole('button', { name: /zalo/i });
    await fireEvent.click(zaloBtn);

    expect(zaloBtn.getAttribute('aria-pressed')).toBe('true');
    expect(onToggle).not.toHaveBeenCalled();
  });

  test('displays message when active channels set is empty', () => {
    render(ChannelBadges, {
      props: {
        channels: mockChannels,
        activeChannels: [],
        onToggle: () => {}
      }
    });

    expect(screen.getByText('No channels currently visible')).toBeTruthy();
  });

  test('renders WhatsApp channel with emerald styling, count badge, and toggles on click', async () => {
    const onToggle = mock((ch: Channel) => {});
    const whatsappChannels: ChannelBadgeSummary[] = [
      { channel: 'whatsapp', messageCount: 42 },
      { channel: 'zalo', messageCount: 8 }
    ];

    const { rerender } = render(ChannelBadges, {
      props: {
        channels: whatsappChannels,
        activeChannels: ['whatsapp', 'zalo'],
        onToggle
      }
    });

    const whatsappBtn = screen.getByRole('button', { name: /whatsapp/i });
    expect(whatsappBtn).toBeTruthy();
    expect(screen.getByText('42')).toBeTruthy();
    expect(whatsappBtn.getAttribute('aria-pressed')).toBe('true');
    expect(whatsappBtn.className).toContain('bg-emerald-600');

    await fireEvent.click(whatsappBtn);
    expect(onToggle).toHaveBeenCalledWith('whatsapp');

    // Rerender with WhatsApp inactive
    await rerender({
      channels: whatsappChannels,
      activeChannels: ['zalo'],
      onToggle
    });

    const inactiveWhatsAppBtn = screen.getByRole('button', { name: /whatsapp/i });
    expect(inactiveWhatsAppBtn.getAttribute('aria-pressed')).toBe('false');
    expect(inactiveWhatsAppBtn.className).toContain('bg-emerald-50');
  });
});
