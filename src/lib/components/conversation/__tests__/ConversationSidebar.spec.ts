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
      { identityId: 100, externalId: 'ext-100', displayName: 'Nguyễn Văn A', avatarUrl: null, participantRole: 'member' }
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
      { identityId: 1, externalId: 'ext-1', displayName: 'Minh', avatarUrl: null, participantRole: 'member' },
      { identityId: 2, externalId: 'ext-2', displayName: 'Thanh', avatarUrl: null, participantRole: 'member' },
      { identityId: 3, externalId: 'ext-3', displayName: 'Hoàng', avatarUrl: null, participantRole: 'member' }
    ]
  };

  const items = [itemCustomer, itemUnknown, itemGroup];

  afterEach(() => {
    cleanup();
  });

  test('renders conversation kind tabs and categorizes customer/unknown vs group rows', () => {
    render(ConversationSidebar, {
      props: {
        items,
        selectedKey: null,
        loading: false,
        onSelect: () => {}
      }
    });

    expect(screen.getByText('Customers (2)')).toBeTruthy();
    expect(screen.getByText('Groups (1)')).toBeTruthy();

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

    const searchInput = screen.getByPlaceholderText(/Search/i);
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

  test('renders UN initials for Unknown contacts with parentheses', () => {
    const itemUnknownFormatted: InboxItem = {
      ...itemUnknown,
      key: 'unknown-3',
      title: 'Unknown (16868895665702)'
    };
    render(ConversationSidebar, {
      props: {
        items: [itemUnknownFormatted],
        selectedKey: null,
        loading: false,
        onSelect: () => {}
      }
    });

    expect(screen.getByText('UN')).toBeTruthy();
    expect(screen.queryByText('U(')).toBeNull();
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

  test('renders conversation kind tabs (All, Customers, Groups) and filters accordingly', async () => {
    const { container } = render(ConversationSidebar, {
      props: {
        items,
        selectedKey: null,
        loading: false,
        onSelect: () => {}
      }
    });

    const tabAll = container.querySelector('[data-kind-tab="all"]');
    const tabCustomer = container.querySelector('[data-kind-tab="customer"]');
    const tabGroup = container.querySelector('[data-kind-tab="group"]');

    expect(tabAll).toBeTruthy();
    expect(tabCustomer).toBeTruthy();
    expect(tabGroup).toBeTruthy();

    // Default 'all' tab shows all items
    expect(screen.getByText('Nguyễn Văn A')).toBeTruthy();
    expect(screen.getByText('0901234567')).toBeTruthy();
    expect(screen.getByText('Nhóm Khách Hàng VIP')).toBeTruthy();

    // Click 'Customers' tab -> shows only customer and unknown items
    await fireEvent.click(tabCustomer!);
    expect(screen.getByText('Nguyễn Văn A')).toBeTruthy();
    expect(screen.getByText('0901234567')).toBeTruthy();
    expect(screen.queryByText('Nhóm Khách Hàng VIP')).toBeNull();

    // Click 'Groups' tab -> shows only group items
    await fireEvent.click(tabGroup!);
    expect(screen.queryByText('Nguyễn Văn A')).toBeNull();
    expect(screen.queryByText('0901234567')).toBeNull();
    expect(screen.getByText('Nhóm Khách Hàng VIP')).toBeTruthy();
  });


  test('sorts unpinned items in All tab strictly by lastMessageAt DESC regardless of kind', () => {
    // itemGroup has lastMessageAt 12:00 (newest)
    // itemUnknown has lastMessageAt 11:00
    // itemCustomer has lastMessageAt 10:00 (oldest)
    render(ConversationSidebar, {
      props: {
        items,
        selectedKey: null,
        loading: false,
        onSelect: () => {}
      }
    });

    const rowElements = screen.getAllByRole('button').filter((el) => el.hasAttribute('data-inbox-item'));
    expect(rowElements.length).toBe(3);

    // Verify ordering: Group (12:00) comes first, then Unknown (11:00), then Customer (10:00)
    expect(rowElements[0].textContent).toContain('Nhóm Khách Hàng VIP');
    expect(rowElements[1].textContent).toContain('0901234567');
    expect(rowElements[2].textContent).toContain('Nguyễn Văn A');
  });

  test('toggles multi-selection mode when Select is clicked and supports select all', async () => {
    const { container } = render(ConversationSidebar, {
      props: {
        items,
        selectedKey: null,
        loading: false,
        onSelect: () => {}
      }
    });

    const selectModeButton = screen.getByText('Select');
    expect(selectModeButton).toBeTruthy();

    await fireEvent.click(selectModeButton);

    // Verify selection mode toolbar appears
    expect(screen.getByText(/Selected 0/)).toBeTruthy();
    const selectAllButton = container.querySelector('[data-select-all]');
    expect(selectAllButton).toBeTruthy();

    // Click select all
    await fireEvent.click(selectAllButton!);
    expect(screen.getByText(/Selected 3/)).toBeTruthy();

    // Exit selection mode
    const exitButton = screen.getByTitle('Exit selection mode');
    await fireEvent.click(exitButton);
    expect(screen.getByText('Select')).toBeTruthy();
  });

  test('only counts and deletes visible items when search query hides selected items', async () => {
    const { container } = render(ConversationSidebar, {
      props: {
        items,
        selectedKey: null,
        loading: false,
        onSelect: () => {}
      }
    });

    // Enter selection mode and select all (3 items)
    await fireEvent.click(screen.getByText('Select'));
    const selectAllButton = container.querySelector('[data-select-all]');
    await fireEvent.click(selectAllButton!);
    expect(screen.getByText(/Selected 3/)).toBeTruthy();

    // Type search query matching only 1 item ("VIP")
    const searchInput = screen.getByPlaceholderText(/Search/i);
    await fireEvent.input(searchInput, { target: { value: 'VIP' } });

    // Counter must update to only count visible selected items (1)
    expect(screen.getByText(/Selected 1/)).toBeTruthy();
    expect(screen.getByText(/Delete \(1\)/)).toBeTruthy();
  });

  test('toggles item selection exactly once on Spacebar press in selection mode', async () => {
    const { container } = render(ConversationSidebar, {
      props: {
        items,
        selectedKey: null,
        loading: false,
        onSelect: () => {}
      }
    });

    // Enter selection mode
    await fireEvent.click(screen.getByText('Select'));
    expect(screen.getByText(/Selected 0/)).toBeTruthy();

    // Target first item row element
    const firstRow = container.querySelector('[data-inbox-item]');
    expect(firstRow).toBeTruthy();

    // Fire Space keydown on the row
    await fireEvent.keyDown(firstRow!, { key: ' ', code: 'Space' });

    // Counter must update to exactly 1 (not 0 caused by double-toggle)
    expect(screen.getByText(/Selected 1/)).toBeTruthy();
  });
});

