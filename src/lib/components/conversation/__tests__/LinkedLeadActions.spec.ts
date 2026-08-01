/// <reference types="bun-types" />
import { afterEach, describe, expect, test, mock } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import type { InboxItem } from '$lib/services/conversationService';
import type { Lead } from '$lib/services/crmService';
import LinkedLeadActions from '../LinkedLeadActions.svelte';

const mockGetLeads = mock(async (): Promise<Lead[]> => [
  {
    id: 1,
    name: 'Nguyễn Văn Lead',
    company: 'Công ty ABC',
    status: 'New',
    value: 0,
    last_activity: '2026-07-31',
    owner: 'Sales',
    email: 'lead@abc.com',
    phone: '0988888888'
  },
  {
    id: 2,
    name: 'Trần Thị Lead',
    company: 'Cá nhân',
    status: 'Qualified',
    value: 500,
    last_activity: '2026-07-30',
    owner: 'Sales',
    email: 'tranthi@gmail.com',
    phone: '0977777777'
  }
]);

const mockLinkConversationIdentity = mock(async (...args: any[]): Promise<void> => {});
const mockUnlinkConversationIdentity = mock(async (...args: any[]): Promise<void> => {});

mock.module('$lib/services/crmService', () => ({
  getLeads: () => mockGetLeads()
}));

mock.module('$lib/services/conversationService', () => ({
  linkConversationIdentity: (...args: any[]) => mockLinkConversationIdentity(...args),
  unlinkConversationIdentity: (...args: any[]) => mockUnlinkConversationIdentity(...args)
}));

describe('LinkedLeadActions Component', () => {
  afterEach(() => {
    cleanup();
    document.body.innerHTML = '';
    mockGetLeads.mockClear();
    mockLinkConversationIdentity.mockClear();
    mockUnlinkConversationIdentity.mockClear();
  });

  const leadItem: InboxItem = {
    key: 'lead-1',
    kind: 'customer',
    leadId: 1,
    conversationId: 55,
    identityId: 500,
    title: 'Nguyễn Văn Lead',
    avatarUrl: null,
    lastMessagePreview: 'Xin chào',
    lastMessageAt: '2026-07-31T10:00:00Z',
    channels: [{ channel: 'zalo', messageCount: 1 }],
    replyTargets: [],
    participantCount: 1,
    participants: []
  };

  test('renders Đổi lead and Hủy liên kết buttons', () => {
    render(LinkedLeadActions, { props: { item: leadItem } });
    expect(screen.getByRole('button', { name: /Đổi lead/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /Hủy liên kết/i })).toBeTruthy();
  });

  test('opens relink dialog and displays CRM leads', async () => {
    render(LinkedLeadActions, { props: { item: leadItem } });

    const relinkBtn = screen.getByRole('button', { name: /Đổi lead/i });
    await fireEvent.click(relinkBtn);
    await tick();

    const heading = screen.getByRole('heading', { name: /Đổi liên kết lead/i });
    expect(heading).toBeTruthy();
    expect(mockGetLeads).toHaveBeenCalledTimes(1);
    expect(screen.getByText('Nguyễn Văn Lead')).toBeTruthy();
    expect(screen.getByText('Trần Thị Lead')).toBeTruthy();
  });

  test('filters leads by search input in relink dialog', async () => {
    render(LinkedLeadActions, { props: { item: leadItem } });

    const relinkBtn = screen.getByRole('button', { name: /Đổi lead/i });
    await fireEvent.click(relinkBtn);
    await tick();

    const searchInput = screen.getByPlaceholderText(/Tìm theo tên, email, sđt/i);
    await fireEvent.input(searchInput, { target: { value: 'Trần Thị' } });
    await tick();

    expect(screen.queryByText('Nguyễn Văn Lead')).toBeNull();
    expect(screen.getByText('Trần Thị Lead')).toBeTruthy();
  });

  test('clicking relink calls linkConversationIdentity and emits onRelinked', async () => {
    const onRelinked = mock(() => {});
    render(LinkedLeadActions, {
      props: {
        item: leadItem,
        onRelinked
      }
    });

    const relinkBtn = screen.getByRole('button', { name: /Đổi lead/i });
    await fireEvent.click(relinkBtn);
    await tick();

    const leadOption = screen.getByText(/Trần Thị Lead/i);
    await fireEvent.click(leadOption);
    await tick();

    const confirmBtn = screen.getByRole('button', { name: /Xác nhận đổi/i });
    await fireEvent.click(confirmBtn);
    await tick();

    expect(mockLinkConversationIdentity).toHaveBeenCalledWith(500, 2);
    expect(onRelinked).toHaveBeenCalledWith(2);
  });

  test('confirming unlink calls unlinkConversationIdentity and emits onUnlinked', async () => {
    const onUnlinked = mock(() => {});
    render(LinkedLeadActions, {
      props: {
        item: leadItem,
        onUnlinked
      }
    });

    const unlinkBtn = screen.getByRole('button', { name: /Hủy liên kết/i });
    await fireEvent.click(unlinkBtn);
    await tick();

    const heading = screen.getByRole('heading', { name: /Xác nhận hủy liên kết/i });
    expect(heading).toBeTruthy();

    const confirmUnlinkBtn = screen.getByRole('button', { name: /Xác nhận hủy/i });
    await fireEvent.click(confirmUnlinkBtn);
    await tick();

    expect(mockUnlinkConversationIdentity).toHaveBeenCalledWith(500);
    expect(onUnlinked).toHaveBeenCalledTimes(1);
  });

  test('disabled when identityId is missing', () => {
    const itemWithoutIdentity: InboxItem = {
      ...leadItem,
      identityId: null
    };

    render(LinkedLeadActions, {
      props: {
        item: itemWithoutIdentity
      }
    });

    const relinkBtn = screen.getByRole('button', { name: /Đổi lead/i }) as HTMLButtonElement;
    const unlinkBtn = screen.getByRole('button', { name: /Hủy liên kết/i }) as HTMLButtonElement;

    expect(relinkBtn.disabled).toBe(true);
    expect(unlinkBtn.disabled).toBe(true);
  });
});
