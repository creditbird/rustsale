/// <reference types="bun-types" />
import { afterEach, describe, expect, test, mock } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import type { InboxItem } from '$lib/services/conversationService';
import type { Lead } from '$lib/services/crmService';
import UnknownContactActions from '../UnknownContactActions.svelte';

const mockGetLeads = mock(async (): Promise<Lead[]> => [
  {
    id: 50,
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
    id: 51,
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

const mockAddLead = mock(async (...args: any[]): Promise<number> => 100);
const mockLinkConversationIdentity = mock(async (...args: any[]): Promise<void> => {});

mock.module('$lib/services/crmService', () => ({
  getLeads: () => mockGetLeads(),
  addLead: (...args: any[]) => mockAddLead(...args)
}));

mock.module('$lib/services/conversationService', () => ({
  linkConversationIdentity: (...args: any[]) => mockLinkConversationIdentity(...args)
}));

describe('UnknownContactActions Component', () => {
  afterEach(() => {
    cleanup();
    document.body.innerHTML = '';
    mockGetLeads.mockClear();
    mockAddLead.mockClear();
    mockLinkConversationIdentity.mockClear();
  });

  const unknownItem: InboxItem = {
    key: 'unknown-1',
    kind: 'unknown',
    leadId: null,
    conversationId: 55,
    identityId: 500,
    title: 'Khách chưa biết',
    avatarUrl: null,
    lastMessagePreview: 'Tư vấn giúp em',
    lastMessageAt: '2026-07-31T10:00:00Z',
    channels: [{ channel: 'zalo', messageCount: 1 }],
    replyTargets: [],
    participantCount: 1,
    participants: []
  };

  test('Link Lead loads CRM leads, filters locally, calls linkConversationIdentity, and emits onLinked(leadId)', async () => {
    const onLinked = mock(() => {});
    render(UnknownContactActions, {
      props: {
        item: unknownItem,
        onLinked
      }
    });

    const linkBtn = screen.getByRole('button', { name: /Link Lead/i });
    await fireEvent.click(linkBtn);
    await tick();

    const heading = screen.getByRole('heading', { name: /Link Existing Lead/i });
    expect(heading).toBeTruthy();
    expect(mockGetLeads).toHaveBeenCalledTimes(1);

    const searchInput = screen.getByPlaceholderText(/Search by name, email, phone/i);
    await fireEvent.input(searchInput, { target: { value: 'Trần Thị' } });
    await tick();

    const leadOption = screen.getByText(/Trần Thị Lead/i);
    await fireEvent.click(leadOption);
    await tick();

    const confirmBtn = screen.getByRole('button', { name: /Confirm Link/i });
    await fireEvent.click(confirmBtn);
    await tick();

    expect(mockLinkConversationIdentity).toHaveBeenCalledWith(500, 51);
    expect(onLinked).toHaveBeenCalledWith(51);
  });

  test('Create Lead pre-fills provider display name, calls crmService.addLead, links returned ID, and emits onLinked', async () => {
    const onLinked = mock(() => {});
    render(UnknownContactActions, {
      props: {
        item: unknownItem,
        onLinked
      }
    });

    const createBtn = screen.getByRole('button', { name: /Create Lead/i });
    await fireEvent.click(createBtn);
    await tick();

    const heading = screen.getByRole('heading', { name: /Create New Lead/i });
    expect(heading).toBeTruthy();

    const nameInput = screen.getByPlaceholderText('Enter lead name...') as HTMLInputElement;
    expect(nameInput.value).toBe('Khách chưa biết');

    const submitBtn = screen.getByRole('button', { name: /Create & Link/i });
    await fireEvent.click(submitBtn);
    await tick();

    expect(mockAddLead).toHaveBeenCalledWith(
      'Khách chưa biết',
      'Individual',
      'New',
      0,
      'Self',
      '',
      '',
      'Created from zalo'
    );
    expect(mockLinkConversationIdentity).toHaveBeenCalledWith(500, 100);
    expect(onLinked).toHaveBeenCalledWith(100);
  });

  test('missing identityId disables both actions', () => {
    const itemWithoutIdentity: InboxItem = {
      ...unknownItem,
      identityId: null
    };

    render(UnknownContactActions, {
      props: {
        item: itemWithoutIdentity
      }
    });

    const linkBtn = screen.getByRole('button', { name: /Link Lead/i }) as HTMLButtonElement;
    const createBtn = screen.getByRole('button', { name: /Create Lead/i }) as HTMLButtonElement;

    expect(linkBtn.disabled).toBe(true);
    expect(createBtn.disabled).toBe(true);
  });

  test('dialogs include accessible titles and validation messages', async () => {
    render(UnknownContactActions, {
      props: {
        item: unknownItem
      }
    });

    const createBtn = screen.getByRole('button', { name: /Create Lead/i });
    await fireEvent.click(createBtn);
    await tick();

    const nameInput = screen.getByPlaceholderText('Enter lead name...');
    await fireEvent.input(nameInput, { target: { value: '   ' } });
    await tick();

    const submitBtn = screen.getByRole('button', { name: /Create & Link/i });
    await fireEvent.click(submitBtn);
    await tick();

    const alertEl = screen.getByRole('alert');
    expect(alertEl).toBeTruthy();
    expect(alertEl.textContent).toContain('Lead name cannot be empty');
    expect(mockAddLead).not.toHaveBeenCalled();
  });
});
