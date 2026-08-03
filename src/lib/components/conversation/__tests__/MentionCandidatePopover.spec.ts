import { afterEach, beforeEach, describe, expect, it, mock } from 'bun:test';
import { cleanup, fireEvent, render, waitFor } from '@testing-library/svelte';
import MentionCandidatePopover from '../MentionCandidatePopover.svelte';
import * as conversationService from '$lib/services/conversationService';

import type { ConversationMentionCandidate } from '$lib/services/conversationService';

const mockList = mock<(_req?: unknown) => Promise<ConversationMentionCandidate[]>>(() =>
  Promise.resolve([])
);

mock.module('$lib/services/conversationService', () => ({
  ...conversationService,
  listConversationMentionCandidates: mockList
}));

describe('MentionCandidatePopover.svelte', () => {
  afterEach(cleanup);

  const mockCandidates = [
    {
      identityId: 1,
      externalId: 'ext-1',
      displayName: 'Nguyễn Văn A',
      avatarUrl: null,
      participantRole: 'admin'
    },
    {
      identityId: 2,
      externalId: 'ext-2',
      displayName: 'Trần Thị B',
      avatarUrl: 'https://example.com/avatar.jpg',
      participantRole: 'member'
    }
  ];

  beforeEach(() => {
    mockList.mockReset();
  });

  it('fetches and renders candidate list on query', async () => {
    mockList.mockResolvedValue(mockCandidates);

    const onSelect = mock();
    const { getByTestId, getByText } = render(MentionCandidatePopover, {
      props: {
        conversationId: 42,
        query: 'Nguyễn',
        selectedIndex: 0,
        currentMentionCount: 0,
        onSelect
      }
    });

    await waitFor(() => {
      expect(mockList).toHaveBeenCalled();
    });

    await waitFor(() => {
      expect(getByText('Nguyễn Văn A')).toBeTruthy();
      expect(getByText('Trần Thị B')).toBeTruthy();
    });

    const item = getByTestId('mention-candidate-1');
    await fireEvent.click(item);
    expect(onSelect).toHaveBeenCalledWith(mockCandidates[0]);
  });

  it('displays warning when max mention count limit (20) is reached', async () => {
    const { getByTestId, getByText } = render(MentionCandidatePopover, {
      props: {
        conversationId: 42,
        query: 'Nguyễn',
        maxMentions: 20,
        currentMentionCount: 20,
        onSelect: mock()
      }
    });

    expect(getByTestId('mention-limit-warning')).toBeTruthy();
    expect(getByText(/Đã đạt giới hạn tối đa 20 nhắc tên/i)).toBeTruthy();
    expect(mockList).not.toHaveBeenCalled();
  });

  it('handles empty candidate search result', async () => {
    mockList.mockResolvedValue([]);

    const { getByTestId } = render(MentionCandidatePopover, {
      props: {
        conversationId: 42,
        query: 'xyz',
        currentMentionCount: 0,
        onSelect: mock()
      }
    });

    await waitFor(() => {
      expect(getByTestId('mention-empty-notice')).toBeTruthy();
    });
  });
});
