import { describe, it, expect, afterEach, mock } from 'bun:test';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import UserProfilePopover from '../UserProfilePopover.svelte';

describe('UserProfilePopover.svelte', () => {
  afterEach(cleanup);

  it('renders user details when open is true and payload is provided', () => {
    const { getByText } = render(UserProfilePopover, {
      props: {
        open: true,
        payload: {
          identityId: 42,
          displayName: 'Nguyen Thai Duy',
          externalId: '123456789',
          role: 'Chủ nhóm'
        }
      }
    });

    expect(getByText('Nguyen Thai Duy')).toBeTruthy();
    expect(getByText('Chủ nhóm')).toBeTruthy();
    expect(getByText('UID: 123456789')).toBeTruthy();
    expect(getByText('Nhắn tin 1-1 (Trò chuyện riêng)')).toBeTruthy();
  });

  it('invokes onOpenDirectChat callback when clicking Nhắn tin 1-1 button', async () => {
    const onOpenDirectChat = mock(() => {});
    const onClose = mock(() => {});

    const { getByTestId } = render(UserProfilePopover, {
      props: {
        open: true,
        payload: {
          identityId: 42,
          displayName: 'Nguyen Thai Duy',
          externalId: '123456789'
        },
        onClose,
        onOpenDirectChat
      }
    });

    const chatBtn = getByTestId('btn-chat-1on1');
    await fireEvent.click(chatBtn);

    expect(onClose).toHaveBeenCalled();
    expect(onOpenDirectChat).toHaveBeenCalledTimes(1);
  });
});
