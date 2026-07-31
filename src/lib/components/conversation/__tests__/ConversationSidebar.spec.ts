/// <reference types="bun-types" />
import { afterEach, beforeAll, describe, expect, test, mock } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import type { InboxItem } from '$lib/services/conversationService';
import ConversationSidebar from '../ConversationSidebar.svelte';

beforeAll(() => {
  if (typeof globalThis.ResizeObserver === 'undefined') {
    globalThis.ResizeObserver = class ResizeObserver {
      observe() {}
      unobserve() {}
      disconnect() {}
    };
  }
});

describe('ConversationSidebar Component', () => {
  const itemCustomer: InboxItem = {
    key: 'customer-1',
    kind: 'customer',
    leadId: 1,
    conversationId: 10,
    identityId: 100,
    title: 'Nguyễn Văn A',
    avatarUrl: 'https://example.com/avatar.jpg',
    lastMessagePreview: 'Xin chào shop',
    lastMessageAt: '2026-07-31T10:00:00Z',
    channels: [
      { channel: 'zalo', messageCount: 5 },
      { channel: 'messenger', messageCount: 2 }
    ],
    replyTargets: [],
    participantCount: 1,
    participants: [
      { identityId: 100, displayName: 'Nguyễn Văn A', avatarUrl: null, participantRole: 'member' }
    ]
  };

  const itemUnknown: InboxItem = {
    key: 'unknown-2',
    kind: 'unknown',
    leadId: null,
    conversationId: 20,
    identityId: 101,
    title: '0901234567',
    avatarUrl: null,
    lastMessagePreview: 'Hỏi giá sản phẩm X',
    lastMessageAt: '2026-07-31T11:00:00Z',
    channels: [
      { channel: 'whatsapp', messageCount: 1 }
    ],
    replyTargets: [],
    participantCount: 1,
    participants: []
  };

  const itemGroup: InboxItem = {
    key: 'group-3',
    kind: 'group',
    leadId: null,
    conversationId: 30,
    identityId: null,
    title: 'Nhóm Khách Hàng VIP',
    avatarUrl: null,
    lastMessagePreview: 'Thông báo họp',
    lastMessageAt: '2026-07-31T12:00:00Z',
    channels: [
      { channel: 'zalo', messageCount: 12 }
    ],
    replyTargets: [],
    participantCount: 3,
    participants: [
      { identityId: 1, displayName: 'Minh', avatarUrl: null, participantRole: 'member' },
      { identityId: 2, displayName: 'Thanh', avatarUrl: null, participantRole: 'member' },
      { identityId: 3, displayName: 'Hoàng', avatarUrl: null, participantRole: 'member' }
    ]
  };

  const items = [itemCustomer, itemUnknown, itemGroup];

  afterEach(() => {
    cleanup();
  });

  test('renders section headers and categorizes customer/unknown vs group rows', () => {
    render(ConversationSidebar, {
      props: {
        items,
        selectedKey: null,
        loading: false,
        onSelect: () => {}
      }
    });

    expect(screen.getByText('Khách hàng')).toBeTruthy();
    expect(screen.getByText('Nhóm')).toBeTruthy();

    expect(screen.getByText('Nguyễn Văn A')).toBeTruthy();
    expect(screen.getByText('0901234567')).toBeTruthy();
    expect(screen.getByText('Nhóm Khách Hàng VIP')).toBeTruthy();
  });

  test('filters rows by title and lastMessagePreview', async () => {
    render(ConversationSidebar, {
      props: {
        items,
        selectedKey: null,
        loading: false,
        onSelect: () => {}
      }
    });

    const searchInput = screen.getByPlaceholderText(/Tìm kiếm/i);
    await fireEvent.input(searchInput, { target: { value: 'Nguyễn' } });

    expect(screen.getByText('Nguyễn Văn A')).toBeTruthy();
    expect(screen.queryByText('0901234567')).toBeNull();
    expect(screen.queryByText('Nhóm Khách Hàng VIP')).toBeNull();

    await fireEvent.input(searchInput, { target: { value: 'họp' } });
    expect(screen.queryByText('Nguyễn Văn A')).toBeNull();
    expect(screen.getByText('Nhóm Khách Hàng VIP')).toBeTruthy();
  });

  test('invokes onSelect when a row is clicked', async () => {
    const onSelect = mock(() => {});
    render(ConversationSidebar, {
      props: {
        items,
        selectedKey: null,
        loading: false,
        onSelect
      }
    });

    const customerRow = screen.getByText('Nguyễn Văn A').closest('[role="button"], button, div[data-inbox-item]');
    expect(customerRow).toBeTruthy();
    await fireEvent.click(customerRow!);

    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith(itemCustomer);
  });

  test('renders channel badges and counts for items', () => {
    render(ConversationSidebar, {
      props: {
        items,
        selectedKey: null,
        loading: false,
        onSelect: () => {}
      }
    });

    expect(screen.getByText('5')).toBeTruthy();
    expect(screen.getByText('2')).toBeTruthy();
    expect(screen.getByText('12')).toBeTruthy();
  });

  test('renders group member count and participant preview names', () => {
    render(ConversationSidebar, {
      props: {
        items,
        selectedKey: null,
        loading: false,
        onSelect: () => {}
      }
    });

    expect(screen.getByText('3')).toBeTruthy();
    expect(screen.getByText(/Minh, Thanh, Hoàng/)).toBeTruthy();
  });

  test('exposes aria-current="true" on selected row', () => {
    render(ConversationSidebar, {
      props: {
        items,
        selectedKey: 'customer-1',
        loading: false,
        onSelect: () => {}
      }
    });

    const selectedRow = screen.getByText('Nguyễn Văn A').closest('[aria-current]');
    expect(selectedRow).toBeTruthy();
    expect(selectedRow?.getAttribute('aria-current')).toBe('true');

    const unselectedRow = screen.getByText('Nhóm Khách Hàng VIP').closest('[aria-current]');
    expect(unselectedRow?.getAttribute('aria-current')).toBe('false');
  });

  test('renders initials when avatarUrl is missing', () => {
    render(ConversationSidebar, {
      props: {
        items,
        selectedKey: null,
        loading: false,
        onSelect: () => {}
      }
    });

    expect(screen.getByText('09')).toBeTruthy();
  });

  test('renders skeletons when loading is true', () => {
    const { container } = render(ConversationSidebar, {
      props: {
        items: [],
        selectedKey: null,
        loading: true,
        onSelect: () => {}
      }
    });

    expect(container.querySelectorAll('.animate-pulse, [data-slot="skeleton"]').length).toBeGreaterThan(0);
  });
});
