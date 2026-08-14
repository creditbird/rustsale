/// <reference types="bun-types" />
import { afterEach, describe, expect, test, mock } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import type { InboxItem } from '$lib/services/conversationService';
import type { Lead, Category } from '$lib/services/crmService';
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

const mockGetCategories = mock(async (): Promise<Category[]> => [
  { id: 10, name: 'Spa & Beauty' },
  { id: 20, name: 'Khách HCM' }
]);

const mockGetCategoriesForLead = mock(async (leadId: number): Promise<Category[]> => {
  if (leadId === 1) return [{ id: 10, name: 'Spa & Beauty' }];
  return [];
});

const mockAddLeadsToCategory = mock(async (leadIds: number[], categoryId: number): Promise<void> => {});
const mockRemoveLeadsFromCategory = mock(async (leadIds: number[], categoryId: number): Promise<void> => {});
const mockCreateCategory = mock(async (name: string): Promise<number> => 30);
const mockOnLeadsChanged = mock((listener: () => void) => () => {});

const mockLinkConversationIdentity = mock(async (...args: any[]): Promise<void> => {});
const mockRelinkConversationLead = mock(async (...args: any[]): Promise<void> => {});
const mockUnlinkConversationIdentity = mock(async (...args: any[]): Promise<void> => {});
const mockUnlinkConversationLead = mock(async (...args: any[]): Promise<void> => {});

mock.module('$lib/services/crmService', () => ({
  getLeads: () => mockGetLeads(),
  getCategories: () => mockGetCategories(),
  getCategoriesForLead: (leadId: number) => mockGetCategoriesForLead(leadId),
  addLeadsToCategory: (leadIds: number[], categoryId: number) => mockAddLeadsToCategory(leadIds, categoryId),
  removeLeadsFromCategory: (leadIds: number[], categoryId: number) => mockRemoveLeadsFromCategory(leadIds, categoryId),
  createCategory: (name: string) => mockCreateCategory(name),
  onLeadsChanged: (listener: () => void) => mockOnLeadsChanged(listener)
}));

mock.module('$lib/services/conversationService', () => ({
  linkConversationIdentity: (...args: any[]) => mockLinkConversationIdentity(...args),
  relinkConversationLead: (...args: any[]) => mockRelinkConversationLead(...args),
  unlinkConversationIdentity: (...args: any[]) => mockUnlinkConversationIdentity(...args),
  unlinkConversationLead: (...args: any[]) => mockUnlinkConversationLead(...args)
}));

describe('LinkedLeadActions Component', () => {
  afterEach(() => {
    cleanup();
    document.body.innerHTML = '';
    mockGetLeads.mockClear();
    mockGetCategories.mockClear();
    mockGetCategoriesForLead.mockClear();
    mockAddLeadsToCategory.mockClear();
    mockRemoveLeadsFromCategory.mockClear();
    mockCreateCategory.mockClear();
    mockLinkConversationIdentity.mockClear();
    mockRelinkConversationLead.mockClear();
    mockUnlinkConversationIdentity.mockClear();
    mockUnlinkConversationLead.mockClear();
  });

  const leadItem: InboxItem = {
    key: 'lead-1',
    kind: 'customer',
    leadId: 1,
    conversationId: null,
    identityId: null,
    title: 'Nguyễn Văn Lead',
    avatarUrl: null,
    lastMessagePreview: 'Xin chào',
    lastMessageAt: '2026-07-31T10:00:00Z',
    channels: [{ channel: 'zalo', messageCount: 1 }],
    replyTargets: [],
    participantCount: 0,
    participants: []
  };

  test('renders Category badge, Category, Change Lead, and Unlink buttons', async () => {
    render(LinkedLeadActions, { props: { item: leadItem } });
    await tick();
    await tick();

    expect(screen.getByText('Spa & Beauty')).toBeTruthy();
    expect(screen.getByRole('button', { name: /Category/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /Change Lead/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /Unlink/i })).toBeTruthy();
  });

  test('opens Category modal and displays categories', async () => {
    render(LinkedLeadActions, { props: { item: leadItem } });
    await tick();

    const categoryBtn = screen.getByRole('button', { name: /Category/i });
    await fireEvent.click(categoryBtn);
    await tick();
    await tick();

    expect(screen.getByRole('heading', { name: /Manage Lead Categories/i })).toBeTruthy();
    expect(mockGetCategories).toHaveBeenCalled();
    expect(screen.getByText('Khách HCM')).toBeTruthy();
  });

  test('toggles category assignment when category item clicked', async () => {
    render(LinkedLeadActions, { props: { item: leadItem } });
    await tick();

    const categoryBtn = screen.getByRole('button', { name: /Category/i });
    await fireEvent.click(categoryBtn);
    await tick();
    await tick();

    const unassignedOption = screen.getByText('Khách HCM');
    await fireEvent.click(unassignedOption);
    await tick();

    expect(mockAddLeadsToCategory).toHaveBeenCalledWith([1], 20);
  });

  test('creates new category and assigns it to current lead', async () => {
    render(LinkedLeadActions, { props: { item: leadItem } });
    await tick();

    const categoryBtn = screen.getByRole('button', { name: /Category/i });
    await fireEvent.click(categoryBtn);
    await tick();
    await tick();

    const input = screen.getByPlaceholderText(/New category name/i);
    await fireEvent.input(input, { target: { value: 'Chiến dịch Mùa Hè' } });
    await tick();

    const createBtn = screen.getByRole('button', { name: /Create/i });
    await fireEvent.click(createBtn);
    await tick();
    await tick();

    expect(mockCreateCategory).toHaveBeenCalledWith('Chiến dịch Mùa Hè');
    expect(mockAddLeadsToCategory).toHaveBeenCalledWith([1], 30);
  });

  test('opens relink dialog and displays CRM leads', async () => {
    render(LinkedLeadActions, { props: { item: leadItem } });

    const relinkBtn = screen.getByRole('button', { name: /Change Lead/i });
    await fireEvent.click(relinkBtn);
    await tick();

    const heading = screen.getByRole('heading', { name: /Change Linked Lead/i });
    expect(heading).toBeTruthy();
    expect(mockGetLeads).toHaveBeenCalledTimes(1);
    expect(screen.getByText('Nguyễn Văn Lead')).toBeTruthy();
    expect(screen.getByText('Trần Thị Lead')).toBeTruthy();
  });

  test('filters leads by search input in relink dialog', async () => {
    render(LinkedLeadActions, { props: { item: leadItem } });

    const relinkBtn = screen.getByRole('button', { name: /Change Lead/i });
    await fireEvent.click(relinkBtn);
    await tick();

    const searchInput = screen.getByPlaceholderText(/Search by name, email, phone/i);
    await fireEvent.input(searchInput, { target: { value: 'Trần Thị' } });
    await tick();

    expect(screen.queryByText('Nguyễn Văn Lead')).toBeNull();
    expect(screen.getByText('Trần Thị Lead')).toBeTruthy();
  });

  test('clicking relink moves the full customer aggregate to the selected lead', async () => {
    const onRelinked = mock(() => {});
    render(LinkedLeadActions, {
      props: {
        item: leadItem,
        onRelinked
      }
    });

    const relinkBtn = screen.getByRole('button', { name: /Change Lead/i });
    await fireEvent.click(relinkBtn);
    await tick();

    const leadOption = screen.getByText(/Trần Thị Lead/i);
    await fireEvent.click(leadOption);
    await tick();

    const confirmBtn = screen.getByRole('button', { name: /Confirm Change/i });
    await fireEvent.click(confirmBtn);
    await tick();

    expect(mockRelinkConversationLead).toHaveBeenCalledWith(1, 2);
    expect(mockLinkConversationIdentity).not.toHaveBeenCalled();
    expect(onRelinked).toHaveBeenCalledWith(2);
  });

  test('confirming unlink calls unlinkConversationLead when leadId is present', async () => {
    const onUnlinked = mock(() => {});
    render(LinkedLeadActions, {
      props: {
        item: leadItem,
        onUnlinked
      }
    });

    const unlinkBtn = screen.getByRole('button', { name: /Unlink/i });
    await fireEvent.click(unlinkBtn);
    await tick();

    const heading = screen.getByRole('heading', { name: /Confirm Unlink/i });
    expect(heading).toBeTruthy();

    const confirmUnlinkBtn = screen.getByRole('button', { name: /Confirm Unlink/i });
    await fireEvent.click(confirmUnlinkBtn);
    await tick();

    expect(mockUnlinkConversationLead).toHaveBeenCalledWith(1);
    expect(onUnlinked).toHaveBeenCalledTimes(1);
  });

  test('confirming unlink calls unlinkConversationIdentity when leadId is null', async () => {
    const onUnlinked = mock(() => {});
    const itemWithoutLead: InboxItem = {
      ...leadItem,
      leadId: null,
      identityId: 500
    };
    render(LinkedLeadActions, {
      props: {
        item: itemWithoutLead,
        onUnlinked
      }
    });

    const unlinkBtn = screen.getByRole('button', { name: /Unlink/i });
    await fireEvent.click(unlinkBtn);
    await tick();

    const confirmUnlinkBtn = screen.getByRole('button', { name: /Confirm Unlink/i });
    await fireEvent.click(confirmUnlinkBtn);
    await tick();

    expect(mockUnlinkConversationIdentity).toHaveBeenCalledWith(500);
    expect(onUnlinked).toHaveBeenCalledTimes(1);
  });
});
