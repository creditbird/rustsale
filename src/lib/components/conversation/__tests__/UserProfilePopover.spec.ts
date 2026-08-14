import { describe, it, expect, afterEach, mock } from 'bun:test';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import UserProfilePopover, { isValidExternalZaloId } from '../UserProfilePopover.svelte';

describe('UserProfilePopover.svelte', () => {
  afterEach(cleanup);

  it('validates Zalo UIDs and phone numbers correctly', () => {
    expect(isValidExternalZaloId('0987654321')).toBe(true);
    expect(isValidExternalZaloId('84987654321')).toBe(true);
    expect(isValidExternalZaloId('481940120149')).toBe(true);
    expect(isValidExternalZaloId('123456789')).toBe(true);
    expect(isValidExternalZaloId('105')).toBe(false);
    expect(isValidExternalZaloId('')).toBe(false);
    expect(isValidExternalZaloId(null)).toBe(false);
  });

  it('renders user details when open is true and payload is provided', () => {
    const { getByText } = render(UserProfilePopover, {
      props: {
        open: true,
        payload: {
          identityId: 42,
          displayName: 'Nguyen Thai Duy',
          externalId: '481940120149',
          role: 'Group Owner'
        }
      }
    });

    expect(getByText('Nguyen Thai Duy')).toBeTruthy();
    expect(getByText('Group Owner')).toBeTruthy();
    expect(getByText('UID: 481940120149')).toBeTruthy();
    expect(getByText('Direct Message (1-on-1 Chat)')).toBeTruthy();
  });

  it('invokes onOpenDirectChat callback when clicking Direct Message button', async () => {
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

  it('invokes onOpenLead callback when clicking Xem / Tạo Lead button', async () => {
    const onOpenLead = mock(() => {});
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
        onOpenLead
      }
    });

    const leadBtn = getByTestId('btn-lead-crm');
    await fireEvent.click(leadBtn);

    expect(onClose).toHaveBeenCalled();
    expect(onOpenLead).toHaveBeenCalledTimes(1);
  });
});
