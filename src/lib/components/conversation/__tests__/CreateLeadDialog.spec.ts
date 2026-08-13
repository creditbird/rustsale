import { describe, it, expect, afterEach, mock } from 'bun:test';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import CreateLeadDialog from '../CreateLeadDialog.svelte';

describe('CreateLeadDialog.svelte', () => {
  afterEach(cleanup);

  it('renders lead creation modal with prefilled name and phone when open is true', () => {
    const { getByText, getByDisplayValue } = render(CreateLeadDialog, {
      props: {
        open: true,
        initialName: 'Luan Nguyen Thanh',
        initialPhoneOrUid: '6045'
      }
    });

    expect(getByText('Tạo Khách Hàng CRM Mới')).toBeTruthy();
    expect(getByDisplayValue('Luan Nguyen Thanh')).toBeTruthy();
    expect(getByDisplayValue('6045')).toBeTruthy();
  });

  it('validates empty name before submitting', async () => {
    const { getByText, getByPlaceholderText } = render(CreateLeadDialog, {
      props: {
        open: true,
        initialName: '',
        initialPhoneOrUid: ''
      }
    });

    const submitBtn = getByText('Lưu Khách Hàng CRM');
    await fireEvent.click(submitBtn);

    expect(getByText('Vui lòng nhập tên khách hàng / Lead')).toBeTruthy();
  });
});
